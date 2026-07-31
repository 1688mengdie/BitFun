/**
 * L1 副本组队→执行→交付→评分全流程验收测试
 *
 * 架构总纲 §12.1 — Layer 3: 端到端测试
 *   - 副本组队→执行→交付→评分全流程
 *
 * 测试场景：
 *   1. 接引台（Gate）场景加载
 *   2. 副本大厅渲染 — 四个副本卡片展示
 *   3. 副本状态标识（招募中/进行中/已关闭）
 *   4. 加入队伍操作与按钮状态切换
 *   5. 队伍成员列表展示
 *   6. 已关闭副本无加入按钮
 */

import { browser, expect, $, $$ } from '@wdio/globals';
import { openScene, expectNoErrorBoundary } from './helpers';

describe('L1 副本组队全流程验收', () => {
  before(async () => {
    await browser.pause(3000);
    await openScene('lvpa-gate');
  });

  it('接引台场景容器应渲染', async () => {
    const scene = await $('[data-scene="lvpa-gate"]');
    await scene.waitForDisplayed({ timeout: 10000 });
    const exists = await scene.isExisting();
    expect(exists).toBe(true);
    console.log('[LVPA] Gate scene rendered');
  });

  it('副本大厅标题和副标题应显示', async () => {
    const title = await $('.party-lobby__title');
    const titleText = await title.getText();
    expect(titleText).toBe('副本大厅');
    console.log('[LVPA] Party lobby title:', titleText);

    const subtitle = await $('.party-lobby__subtitle');
    const subtitleExists = await subtitle.isExisting();
    expect(subtitleExists).toBe(true);
  });

  it('应显示四个副本卡片', async () => {
    const dungeons = await $$('.party-lobby__dungeon');
    console.log('[LVPA] Dungeon cards count:', dungeons.length);
    expect(dungeons.length).toBe(4);
  });

  it('四个副本名称应正确', async () => {
    const dungeonNames = await $$('.party-lobby__dungeon-name');
    const names = await Promise.all(dungeonNames.map((el) => el.getText()));
    console.log('[LVPA] Dungeon names:', names);
    expect(names).toContain('BTC 日线秘境');
    expect(names).toContain('ETH 合约战场');
    expect(names).toContain('多币种矩阵');
    expect(names).toContain('量化飞升试炼');
  });

  it('副本状态标识应正确', async () => {
    const statusBadges = await $$('.party-lobby__status-badge');
    const statusTexts = await Promise.all(statusBadges.map((el) => el.getText()));
    console.log('[LVPA] Dungeon statuses:', statusTexts);

    expect(statusTexts).toContain('招募中');
    expect(statusTexts).toContain('进行中');
    expect(statusTexts).toContain('已关闭');
  });

  it('招募中的副本应显示加入队伍按钮', async () => {
    const dungeons = await $$('.party-lobby__dungeon');
    let found = false;

    for (const dungeon of dungeons) {
      const hasStatusOpen = await dungeon.$('.party-lobby__status-badge--open').isExisting();
      if (!hasStatusOpen) continue;

      const joinBtn = await dungeon.$('.party-lobby__join-btn');
      const btnExists = await joinBtn.isExisting();
      if (btnExists) {
        const btnText = await joinBtn.getText();
        console.log('[LVPA] Join button text:', btnText);
        expect(btnText).toBe('加入队伍');
        found = true;
        break;
      }
    }

    expect(found).toBe(true);
  });

  it('加入队伍按钮点击可切换为退出队伍', async () => {
    const dungeons = await $$('.party-lobby__dungeon');
    let clicked = false;

    for (const dungeon of dungeons) {
      const joinBtn = await dungeon.$('.party-lobby__join-btn');
      const btnExists = await joinBtn.isExisting();
      if (!btnExists) continue;

      const btnText = await joinBtn.getText();
      if (btnText !== '加入队伍') continue;

      // 点击加入队伍
      await joinBtn.click();
      await browser.pause(500);

      // 按钮应变为"退出队伍"
      const newBtnText = await joinBtn.getText();
      console.log('[LVPA] Join button after click:', newBtnText);
      expect(newBtnText).toBe('退出队伍');

      // 点击退出队伍
      await joinBtn.click();
      await browser.pause(500);

      // 恢复为"加入队伍"
      const restoredText = await joinBtn.getText();
      console.log('[LVPA] Join button restored:', restoredText);
      expect(restoredText).toBe('加入队伍');

      clicked = true;
      break;
    }

    expect(clicked).toBe(true);
  });

  it('进行中的副本应显示队伍成员', async () => {
    const dungeons = await $$('.party-lobby__dungeon');

    for (const dungeon of dungeons) {
      const hasStatusInProgress = await dungeon.$('.party-lobby__status-badge--in_progress').isExisting();
      if (!hasStatusInProgress) continue;

      const partySection = await dungeon.$('.party-lobby__party');
      const partyExists = await partySection.isExisting();
      console.log('[LVPA] Party section exists for in-progress dungeon:', partyExists);
      expect(partyExists).toBe(true);

      const members = await dungeon.$$('.party-lobby__member');
      console.log('[LVPA] Party members count:', members.length);
      expect(members.length).toBeGreaterThanOrEqual(1);
      return;
    }
  });

  it('已关闭副本应无加入按钮', async () => {
    const dungeons = await $$('.party-lobby__dungeon');

    for (const dungeon of dungeons) {
      const hasStatusClosed = await dungeon.$('.party-lobby__status-badge--closed').isExisting();
      if (!hasStatusClosed) continue;

      const joinBtn = await dungeon.$('.party-lobby__join-btn');
      const exists = await joinBtn.isExisting();
      console.log('[LVPA] Join button exists for closed dungeon:', exists);
      expect(exists).toBe(false);
      return;
    }
  });

  it('无错误边界', async () => {
    await expectNoErrorBoundary();
  });
});
