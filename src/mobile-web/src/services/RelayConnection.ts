/**
 * WebSocket connection to the relay server from the mobile client.
 * Handles join_room, message relay, heartbeat, reconnection,
 * and HTTP polling for buffered messages.
 */

import { generateKeyPair, deriveSharedKey, encrypt, decrypt, toB64, fromB64, MobileKeyPair } from './E2EEncryption';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'paired' | 'error';

export interface RelayCallbacks {
  onStateChange: (state: ConnectionState) => void;
  onMessage: (json: string) => void;
  onError: (msg: string) => void;
}

export interface BufferedMessage {
  seq: number;
  timestamp: number;
  direction: string;
  encrypted_data: string;
  nonce: string;
}

export class RelayConnection {
  private ws: WebSocket | null = null;
  private keyPair: MobileKeyPair | null = null;
  private sharedKey: Uint8Array | null = null;
  private roomId: string;
  private desktopPubKey: Uint8Array;
  private deviceId: string;
  private callbacks: RelayCallbacks;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private reconnectAttempts = 0;
  private maxReconnects = 5;
  private wsUrl: string;
  private httpBaseUrl: string;
  private messageQueue: Promise<void> = Promise.resolve();
  private destroyed = false;
  private _lastSeq = 0;
  private pollTimer: ReturnType<typeof setInterval> | null = null;

  constructor(
    wsUrl: string,
    roomId: string,
    desktopPubKeyB64: string,
    desktopDeviceId: string,
    callbacks: RelayCallbacks,
  ) {
    this.wsUrl = wsUrl;
    this.roomId = roomId;
    this.desktopPubKey = fromB64(desktopPubKeyB64);
    this.deviceId = desktopDeviceId;
    this.callbacks = callbacks;

    // Derive HTTP base URL from wsUrl for polling
    this.httpBaseUrl = wsUrl
      .replace(/^wss:\/\//, 'https://')
      .replace(/^ws:\/\//, 'http://')
      .replace(/\/ws\/?$/, '')
      .replace(/\/$/, '');
  }

  get lastSeq(): number {
    return this._lastSeq;
  }

  async connect() {
    this.callbacks.onStateChange('connecting');
    this.keyPair = await generateKeyPair();

    try {
      this.sharedKey = await deriveSharedKey(this.keyPair, this.desktopPubKey);
    } catch (e: any) {
      this.callbacks.onError(`Key derivation failed: ${e?.message || e}`);
      return;
    }

    let url: string;
    if (this.wsUrl.startsWith('ws://') || this.wsUrl.startsWith('wss://')) {
      url = this.wsUrl.replace(/\/$/, '') + '/ws';
    } else {
      url = this.wsUrl.replace(/^https?:\/\//, (m) =>
        m.startsWith('https') ? 'wss://' : 'ws://',
      ).replace(/\/$/, '') + '/ws';
    }

    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.callbacks.onStateChange('connected');
      this.sendJson({
        type: 'join_room',
        room_id: this.roomId,
        device_id: `mobile-${Date.now().toString(36)}`,
        device_type: 'mobile',
        public_key: toB64(this.keyPair!.publicKey),
      });
      this.startHeartbeat();
    };

    this.ws.onmessage = (ev) => {
      this.messageQueue = this.messageQueue.then(async () => {
        try {
          const msg = JSON.parse(ev.data);
          await this.handleMessage(msg);
        } catch (e) {
          console.error('[Relay] Failed to handle message', e);
        }
      });
    };

    this.ws.onclose = () => {
      this.stopHeartbeat();
      if (!this.destroyed && this.reconnectAttempts < this.maxReconnects) {
        this.reconnectAttempts++;
        const delay = 1000 * this.reconnectAttempts;
        setTimeout(() => this.connect(), delay);
      } else {
        this.callbacks.onStateChange('disconnected');
      }
    };

    this.ws.onerror = () => {
      this.callbacks.onError('WebSocket connection error');
    };
  }

  private async handleMessage(msg: any) {
    switch (msg.type) {
      case 'peer_joined': {
        this.reconnectAttempts = 0;
        break;
      }

      case 'relay': {
        if (!this.sharedKey) {
          this.callbacks.onError('Received relay before key exchange completed');
          return;
        }
        try {
          const plaintext = await decrypt(this.sharedKey, msg.encrypted_data, msg.nonce);
          const parsed = JSON.parse(plaintext);
          if (parsed.challenge && parsed.timestamp) {
            const response = JSON.stringify({
              challenge_echo: parsed.challenge,
              device_id: `mobile-${Date.now().toString(36)}`,
              device_name: this.getMobileDeviceName(),
            });
            await this.sendEncrypted(response);
            this.callbacks.onStateChange('paired');
          } else {
            this.callbacks.onMessage(plaintext);
          }
        } catch (e: any) {
          this.callbacks.onError(`Decrypt failed: ${e?.message || e}`);
        }
        break;
      }

      case 'peer_disconnected':
        this.callbacks.onStateChange('disconnected');
        break;

      case 'heartbeat_ack':
        break;

      case 'error':
        this.callbacks.onError(msg.message || 'relay error');
        break;
    }
  }

  async sendEncrypted(plaintext: string) {
    if (!this.sharedKey || !this.ws) return;
    const { data, nonce } = await encrypt(this.sharedKey, plaintext);
    this.sendJson({
      type: 'relay',
      room_id: this.roomId,
      encrypted_data: data,
      nonce,
    });
  }

  async sendCommand(cmd: object) {
    await this.sendEncrypted(JSON.stringify(cmd));
  }

  private sendJson(obj: any) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(obj));
    }
  }

  private startHeartbeat() {
    this.heartbeatTimer = setInterval(() => {
      this.sendJson({ type: 'heartbeat' });
    }, 30_000);
  }

  private stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  private getMobileDeviceName(): string {
    const ua = navigator.userAgent;
    if (/iPhone/i.test(ua)) return 'iPhone';
    if (/iPad/i.test(ua)) return 'iPad';
    if (/Android/i.test(ua)) return 'Android';
    return 'Mobile Browser';
  }

  setMessageHandler(handler: (json: string) => void) {
    this.callbacks.onMessage = handler;
  }

  // ── HTTP Polling API ──────────────────────────────────────────

  /** Poll the relay server for buffered messages via HTTP. */
  async pollMessages(): Promise<BufferedMessage[]> {
    try {
      const url = `${this.httpBaseUrl}/api/rooms/${encodeURIComponent(this.roomId)}/poll?since_seq=${this._lastSeq}&device_type=mobile`;
      const resp = await fetch(url);
      if (!resp.ok) return [];
      const data = await resp.json();
      const messages: BufferedMessage[] = data.messages || [];

      if (messages.length > 0) {
        const maxSeq = Math.max(...messages.map((m: BufferedMessage) => m.seq));
        this._lastSeq = maxSeq;
      }
      return messages;
    } catch {
      return [];
    }
  }

  /** Acknowledge receipt of messages up to the current lastSeq. */
  async ackMessages(): Promise<void> {
    if (this._lastSeq === 0) return;
    try {
      await fetch(`${this.httpBaseUrl}/api/rooms/${encodeURIComponent(this.roomId)}/ack`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ack_seq: this._lastSeq, device_type: 'mobile' }),
      });
    } catch {
      // best effort
    }
  }

  /** Start periodic polling (call after pairing). */
  startPolling(intervalMs = 2000) {
    this.stopPolling();
    this.pollTimer = setInterval(async () => {
      const messages = await this.pollMessages();
      if (!this.sharedKey) return;
      for (const msg of messages) {
        try {
          const plaintext = await decrypt(this.sharedKey, msg.encrypted_data, msg.nonce);
          this.callbacks.onMessage(plaintext);
        } catch {
          // skip messages that fail to decrypt
        }
      }
      if (messages.length > 0) {
        await this.ackMessages();
      }
    }, intervalMs);
  }

  stopPolling() {
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  disconnect() {
    this.destroyed = true;
    this.stopHeartbeat();
    this.stopPolling();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.sharedKey = null;
    this.callbacks.onStateChange('disconnected');
  }

  get isPaired(): boolean {
    return this.sharedKey !== null;
  }
}
