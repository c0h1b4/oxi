"use client";

import { forwardRef, useMemo } from "react";
import { motion, type MotionProps, type Variants } from "framer-motion";
import { useUiStore } from "@/stores/useUiStore";

// Props that conflict between HTML div and framer-motion's motion.div
// (framer-motion redefines onDrag/onDragStart/onDragEnd with different signatures).
type ConflictingMotionProps = "onDrag" | "onDragStart" | "onDragEnd" | "onAnimationStart";

type HTMLDivProps = Omit<React.ComponentPropsWithoutRef<"div">, ConflictingMotionProps>;

type AnimatedDivOwnProps = {
  variants?: Variants;
  initial?: MotionProps["initial"];
  animate?: MotionProps["animate"];
  exit?: MotionProps["exit"];
  transition?: MotionProps["transition"];
  layout?: MotionProps["layout"];
  layoutId?: string;
  /** Forwarded verbatim as data-testid on the rendered element. */
  "data-testid"?: string;
  /**
   * When true the component serialises `variants` into a `data-motion-props`
   * attribute so tests can assert on the motion configuration.  Defaults to
   * true when `variants` is provided.
   */
  exposeMotionProps?: boolean;
};

export type AnimatedDivProps = HTMLDivProps & AnimatedDivOwnProps;

/**
 * Renders `motion.div` with the supplied animation props when the global
 * animation mode is not "off", otherwise renders a plain `<div>` forwarding
 * only the standard HTML attributes.
 *
 * Reading `effectiveAnimationMode` from the Zustand store happens internally
 * so call-sites no longer need the boilerplate:
 *
 * ```ts
 * const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
 * const shouldAnimate = effectiveAnimationMode !== "off";
 * ```
 */
export const AnimatedDiv = forwardRef<HTMLDivElement, AnimatedDivProps>(
  function AnimatedDiv(
    {
      variants,
      initial,
      animate,
      exit,
      transition,
      layout,
      layoutId,
      exposeMotionProps,
      ...htmlProps
    },
    ref,
  ) {
    const effectiveAnimationMode = useUiStore(
      (s) => s.effectiveAnimationMode,
    );
    const shouldAnimate = effectiveAnimationMode !== "off";

    const dataMotionProps = useMemo(() => {
      const shouldExpose = exposeMotionProps ?? !!variants;
      if (!shouldExpose || !variants) return undefined;
      return JSON.stringify(variants);
    }, [exposeMotionProps, variants]);

    if (!shouldAnimate) {
      // Strip data-testid in off mode — the old animated/static branch pattern
      // only placed transition test IDs on the animated branch, and tests assert
      // their absence when animations are off.
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const { "data-testid": _strippedTestId, ...staticProps } = htmlProps;
      return (
        <div
          ref={ref}
          {...staticProps}
        />
      );
    }

    return (
      <motion.div
        ref={ref}
        variants={variants}
        initial={initial}
        animate={animate}
        exit={exit}
        transition={transition}
        layout={layout}
        layoutId={layoutId}
        data-motion-props={dataMotionProps}
        {...htmlProps}
      />
    );
  },
);
