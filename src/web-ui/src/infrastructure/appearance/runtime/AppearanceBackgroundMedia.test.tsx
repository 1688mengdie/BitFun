import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { AppearanceBackgroundMediaLayer } from './AppearanceBackgroundMedia';

describe('AppearanceBackgroundMedia', () => {
  it('renders a muted looping video with a host-owned poster fallback', () => {
    const html = renderToStaticMarkup(<AppearanceBackgroundMediaLayer media={{
      kind: 'video',
      assetId: 'motion',
      posterAssetId: 'poster',
      fit: 'cover',
      position: 'center',
      url: 'blob:test-motion',
      posterUrl: 'blob:test-poster',
    }} />);

    expect(html).toContain('data-bf-background-media="video"');
    expect(html).toContain('src="blob:test-motion"');
    expect(html).toContain('poster="blob:test-poster"');
    expect(html).toContain('autoplay=""');
    expect(html).toContain('loop=""');
    expect(html).toContain('muted=""');
  });
});
