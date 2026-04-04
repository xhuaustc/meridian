import { useEffect, useState } from 'react';
import { getPlatform } from '../lib/api';

export type Platform = 'macos' | 'windows' | 'linux';

let cached: Platform | null = null;

export function usePlatform(): Platform | null {
  const [platform, setPlatform] = useState<Platform | null>(cached);

  useEffect(() => {
    if (cached) return;
    getPlatform().then((p) => {
      cached = p as Platform;
      setPlatform(cached);
    });
  }, []);

  return platform;
}
