import yaml from 'yaml';

export interface IdentityDocument {
  name: string;
  creature: string;
  vibe: string;
  emoji: string;
  body: string;
}

export const EMPTY_IDENTITY_DOCUMENT: IdentityDocument = {
  name: '',
  creature: '',
  vibe: '',
  emoji: '',
  body: '',
};

const FRONTMATTER_FIELDS: Array<keyof Omit<IdentityDocument, 'body'>> = [
  'name',
  'creature',
  'vibe',
  'emoji',
];

function normalizeLineEndings(content: string): string {
  return content.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
}

function normalizeShortField(value: unknown): string {
  if (typeof value !== 'string') {
    return '';
  }

  return value.replace(/\s+/g, ' ').trim();
}

function serializeScalar(value: string): string {
  return yaml.stringify(value).trimEnd();
}

export function parseIdentityDocument(content: string): IdentityDocument {
  const normalizedContent = normalizeLineEndings(content || '');
  const frontmatterMatch = normalizedContent.match(/^---\n([\s\S]*?)\n---\n?([\s\S]*)$/);

  if (!frontmatterMatch) {
    return {
      ...EMPTY_IDENTITY_DOCUMENT,
      body: normalizedContent.trim(),
    };
  }

  const parsed = (yaml.parse(frontmatterMatch[1]) || {}) as Record<string, unknown>;
  const body = frontmatterMatch[2] ?? '';

  return {
    name: normalizeShortField(parsed.name),
    creature: normalizeShortField(parsed.creature),
    vibe: normalizeShortField(parsed.vibe),
    emoji: normalizeShortField(parsed.emoji),
    body: body.replace(/^\n+/, '').trimEnd(),
  };
}

export function serializeIdentityDocument(document: IdentityDocument): string {
  const normalized = {
    name: normalizeShortField(document.name),
    creature: normalizeShortField(document.creature),
    vibe: normalizeShortField(document.vibe),
    emoji: normalizeShortField(document.emoji),
    body: normalizeLineEndings(document.body || '').replace(/^\n+/, '').trimEnd(),
  };

  const frontmatter = FRONTMATTER_FIELDS.map((field) => {
    const value = normalized[field];
    return value ? `${field}: ${serializeScalar(value)}` : `${field}:`;
  }).join('\n');

  return `---\n${frontmatter}\n---\n\n${normalized.body}`.trimEnd() + '\n';
}

export function getIdentityFilePath(workspaceRoot: string): string {
  const normalizedRoot = workspaceRoot.replace(/\\/g, '/').replace(/\/+$/, '');
  return `${normalizedRoot}/IDENTITY.md`;
}
