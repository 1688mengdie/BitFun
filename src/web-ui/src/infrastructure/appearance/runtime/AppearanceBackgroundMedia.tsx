import { useEffect, useRef } from 'react';
import type { ResolvedAppearanceBackgroundMedia } from '../types';

export function AppearanceBackgroundMediaLayer({
  media,
}: {
  media: Readonly<ResolvedAppearanceBackgroundMedia> | undefined;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !media?.url) return undefined;
    const reducedMotion = window.matchMedia?.('(prefers-reduced-motion: reduce)') ?? null;
    const synchronizePlayback = (): void => {
      if (document.hidden || reducedMotion?.matches) {
        video.pause();
        return;
      }
      void video.play().catch(() => undefined);
    };
    document.addEventListener('visibilitychange', synchronizePlayback);
    reducedMotion?.addEventListener?.('change', synchronizePlayback);
    synchronizePlayback();
    return () => {
      document.removeEventListener('visibilitychange', synchronizePlayback);
      reducedMotion?.removeEventListener?.('change', synchronizePlayback);
      video.pause();
    };
  }, [media?.url]);

  if (!media?.url || !media.posterUrl) return null;
  return (
    <div
      className="bitfun-appearance-background-media"
      aria-hidden="true"
      data-bf-background-media="video"
      style={{
        backgroundImage: `url("${media.posterUrl}")`,
        backgroundPosition: media.position ?? 'center',
        backgroundSize: media.fit ?? 'cover',
      }}
    >
      <video
        ref={videoRef}
        className="bitfun-appearance-background-media__video"
        src={media.url}
        poster={media.posterUrl}
        autoPlay
        loop
        muted
        playsInline
        preload="auto"
        disablePictureInPicture
        tabIndex={-1}
        style={{
          objectFit: media.fit ?? 'cover',
          objectPosition: media.position ?? 'center',
        }}
      />
    </div>
  );
}
