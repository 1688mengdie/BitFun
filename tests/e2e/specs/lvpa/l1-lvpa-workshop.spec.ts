/**
 * L1 工坊工作流验收测试
 *
 * 架构总纲 §12.1 — Layer 3: 端到端测试
 *   - 完整工坊工作流验收
 *
 * 测试场景：
 *   1. 工坊场景加载和渲染
 *   2. 四个工坊卡片展示（天机坊/金算坊/丹青坊/留影坊）
 *   3. 工坊卡片选中与 DAG 工作流渲染
 *   4. 工坊状态标识（运转中/暂停/空闲）
 *   5. DAG 节点状态展示
 */

import { browser, expect, $, $$ } from '@wdio/globals';
import { openScene, expectNoErrorBoundary } from './helpers';

describe('L1 工坊工作流验收', () => {
  before(async () => {
    await browser.pause(3000);
    await openScene('lvpa-workshop');
  });

  it('工坊场景容器应渲染', async () => {
    const scene = await $('[data-scene="lvpa-workshop"]');
    await scene.waitForDisplayed({ timeout: 10000 });
    const exists = await scene.isExisting();
    expect(exists).toBe(true);
    console.log('[LVPA] Workshop scene rendered');
  });

  it('工坊标题和副标题应显示', async () => {
    const title = await $('.workshop-dag__title');
    const titleText = await title.getText();
    expect(titleText).toBe('工坊');
    console.log('[LVPA] Workshop title:', titleText);

    const subtitle = await $('.workshop-dag__subtitle');
    const subtitleExists = await subtitle.isExisting();
    expect(subtitleExists).toBe(true);
  });

  it('应显示四个工坊卡片', async () => {
    const cards = await $$('.workshop-dag__card');
    console.log('[LVPA] Workshop cards count:', cards.length);
    expect(cards.length).toBe(4);
  });

  it('四个工坊名称应正确', async () => {
    const cardNames = await $$('.workshop-dag__card-name');
    const names = await Promise.all(cardNames.map((el) => el.getText()));
    console.log('[LVPA] Card names:', names);
    expect(names).toContain('天机坊');
    expect(names).toContain('金算坊');
    expect(names).toContain('丹青坊');
    expect(names).toContain('留影坊');
  });

  it('天机坊默认选中并显示 DAG 工作流', async () => {
    // 天机坊应默认选中（selectedId = 'tianji'）
    const selectedCard = await $('.workshop-dag__card--selected');
    await selectedCard.waitForDisplayed({ timeout: 5000 });

    const selectedName = await selectedCard.$('.workshop-dag__card-name').getText();
    console.log('[LVPA] Selected workshop:', selectedName);
    expect(selectedName).toContain('天机坊');

    // DAG 区域应渲染
    const dagArea = await $('.workshop-dag__dag');
    const dagExists = await dagArea.isExisting();
    expect(dagExists).toBe(true);

    const dagTitle = await $('.workshop-dag__dag-title');
    const dagTitleText = await dagTitle.getText();
    console.log('[LVPA] DAG title:', dagTitleText);
    expect(dagTitleText).toContain('天机坊');
    expect(dagTitleText).toContain('工作流');

    // DAG 节点应存在
    const dagNodes = await $$('.workshop-dag__dag-node-box');
    console.log('[LVPA] DAG nodes count:', dagNodes.length);
    expect(dagNodes.length).toBeGreaterThan(0);
  });

  it('点击切换工坊卡片应更新 DAG', async () => {
    // 点击金算坊
    const cards = await $$('.workshop-dag__card');
    let jinsuanCard: WebdriverIO.Element | null = null;
    for (const card of cards) {
      const text = await card.getText();
      if (text.includes('金算坊')) {
        jinsuanCard = card;
        break;
      }
    }
    expect(jinsuanCard).not.toBeNull();
    await jinsuanCard!.click();
    await browser.pause(500);

    // 验证金算坊被选中
    const selectedCard = await $('.workshop-dag__card--selected');
    const selectedName = await selectedCard.$('.workshop-dag__card-name').getText();
    expect(selectedName).toContain('金算坊');

    // DAG 标题应更新
    const dagTitle = await $('.workshop-dag__dag-title');
    const dagTitleText = await dagTitle.getText();
    console.log('[LVPA] Updated DAG title:', dagTitleText);
    expect(dagTitleText).toContain('金算坊');
  });

  it('工坊状态标识应正确显示', async () => {
    const statusBadges = await $$('.workshop-dag__card-status');
    const statusTexts = await Promise.all(statusBadges.map((el) => el.getText()));
    console.log('[LVPA] Workshop statuses:', statusTexts);

    expect(statusTexts).toContain('运转中');
    expect(statusTexts).toContain('暂停');
    expect(statusTexts).toContain('空闲');
  });

  it('运转中的工坊应显示进度条', async () => {
    const progressBars = await $$('.workshop-dag__progress-bar');
    console.log('[LVPA] Progress bars count:', progressBars.length);
    // 天机坊和金算坊有 currentProject，应显示进度条
    expect(progressBars.length).toBeGreaterThanOrEqual(1);
  });

  it('无错误边界', async () => {
    await expectNoErrorBoundary();
  });
});
