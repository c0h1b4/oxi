export const ANIMATION_MODES = ["rich", "medium", "subtle", "off"] as const;

export type AnimationMode = (typeof ANIMATION_MODES)[number];

export type MotionTokens = {
  duration: {
    instant: number;
    fast: number;
    normal: number;
    slow: number;
  };
  distance: {
    xs: number;
    sm: number;
    md: number;
  };
  spring: {
    stiffness: number;
    damping: number;
    mass: number;
  };
  stagger: {
    list: number;
  };
};

export type MotionConfig = {
  storedMode: AnimationMode | null;
  effectiveMode: AnimationMode;
  isOff: boolean;
  isReducedBySystem: boolean;
  tokens: MotionTokens;
};

export const DEFAULT_UNSET_NON_REDUCED_MODE: AnimationMode = "medium";

const TOKENS_BY_MODE: Record<AnimationMode, MotionTokens> = {
  rich: {
    duration: { instant: 0.08, fast: 0.18, normal: 0.28, slow: 0.42 },
    distance: { xs: 4, sm: 10, md: 16 },
    spring: { stiffness: 260, damping: 24, mass: 0.95 },
    stagger: { list: 0.045 },
  },
  medium: {
    duration: { instant: 0.06, fast: 0.14, normal: 0.22, slow: 0.32 },
    distance: { xs: 3, sm: 8, md: 12 },
    spring: { stiffness: 300, damping: 28, mass: 0.9 },
    stagger: { list: 0.03 },
  },
  subtle: {
    duration: { instant: 0.04, fast: 0.1, normal: 0.16, slow: 0.24 },
    distance: { xs: 2, sm: 5, md: 8 },
    spring: { stiffness: 360, damping: 32, mass: 0.85 },
    stagger: { list: 0.015 },
  },
  off: {
    duration: { instant: 0, fast: 0, normal: 0, slow: 0 },
    distance: { xs: 0, sm: 0, md: 0 },
    spring: { stiffness: 1000, damping: 1000, mass: 1 },
    stagger: { list: 0 },
  },
};

function cloneTokens(tokens: MotionTokens): MotionTokens {
  return {
    duration: { ...tokens.duration },
    distance: { ...tokens.distance },
    spring: { ...tokens.spring },
    stagger: { ...tokens.stagger },
  };
}

export function isAnimationMode(value: unknown): value is AnimationMode {
  return typeof value === "string" && (ANIMATION_MODES as readonly string[]).includes(value);
}

export function resolveAnimationMode(
  storedMode: unknown,
  prefersReducedMotion: boolean,
): AnimationMode {
  if (isAnimationMode(storedMode)) {
    return storedMode;
  }

  return prefersReducedMotion ? "off" : DEFAULT_UNSET_NON_REDUCED_MODE;
}

export function getMotionTokens(mode: AnimationMode): MotionTokens {
  return cloneTokens(TOKENS_BY_MODE[mode]);
}

export function resolveMotionConfig(
  storedMode: unknown,
  prefersReducedMotion: boolean,
  resolver: (stored: unknown, reduced: boolean) => AnimationMode = resolveAnimationMode,
): MotionConfig {
  try {
    const normalizedStoredMode = isAnimationMode(storedMode) ? storedMode : null;
    const effectiveMode = resolver(storedMode, prefersReducedMotion);

    return {
      storedMode: normalizedStoredMode,
      effectiveMode,
      isOff: effectiveMode === "off",
      isReducedBySystem: prefersReducedMotion,
      tokens: getMotionTokens(effectiveMode),
    };
  } catch {
    return {
      storedMode: null,
      effectiveMode: "off",
      isOff: true,
      isReducedBySystem: prefersReducedMotion,
      tokens: getMotionTokens("off"),
    };
  }
}
