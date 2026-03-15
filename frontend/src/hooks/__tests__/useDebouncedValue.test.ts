import { describe, it, expect, vi, beforeEach, afterEach } from "vitest"
import { renderHook, act } from "@testing-library/react"
import { useDebouncedValue } from "../useDebouncedValue"

describe("useDebouncedValue", () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("returns initial value immediately", () => {
    const { result } = renderHook(() => useDebouncedValue("initial", 200))
    expect(result.current).toBe("initial")
  })

  it("does not update value before delay elapses", () => {
    const { result, rerender } = renderHook(
      ({ value, delay }) => useDebouncedValue(value, delay),
      { initialProps: { value: "first", delay: 200 } }
    )

    expect(result.current).toBe("first")

    rerender({ value: "second", delay: 200 })
    expect(result.current).toBe("first")

    act(() => {
      vi.advanceTimersByTime(100)
    })
    expect(result.current).toBe("first")
  })

  it("updates value after delay elapses", () => {
    const { result, rerender } = renderHook(
      ({ value, delay }) => useDebouncedValue(value, delay),
      { initialProps: { value: "first", delay: 200 } }
    )

    rerender({ value: "second", delay: 200 })

    act(() => {
      vi.advanceTimersByTime(200)
    })
    expect(result.current).toBe("second")
  })

  it("cancels pending update when value changes before delay", () => {
    const { result, rerender } = renderHook(
      ({ value, delay }) => useDebouncedValue(value, delay),
      { initialProps: { value: "first", delay: 200 } }
    )

    rerender({ value: "second", delay: 200 })
    act(() => {
      vi.advanceTimersByTime(100)
    })

    rerender({ value: "third", delay: 200 })
    act(() => {
      vi.advanceTimersByTime(100)
    })

    expect(result.current).toBe("first")

    act(() => {
      vi.advanceTimersByTime(100)
    })
    expect(result.current).toBe("third")
  })

  it("uses custom delay value", () => {
    const { result, rerender } = renderHook(
      ({ value, delay }) => useDebouncedValue(value, delay),
      { initialProps: { value: "initial", delay: 500 } }
    )

    rerender({ value: "updated", delay: 500 })

    act(() => {
      vi.advanceTimersByTime(200)
    })
    expect(result.current).toBe("initial")

    act(() => {
      vi.advanceTimersByTime(300)
    })
    expect(result.current).toBe("updated")
  })

  it("defaults to 200ms delay when not specified", () => {
    const { result, rerender } = renderHook(
      ({ value }) => useDebouncedValue(value),
      { initialProps: { value: "initial" } }
    )

    rerender({ value: "updated" })

    act(() => {
      vi.advanceTimersByTime(199)
    })
    expect(result.current).toBe("initial")

    act(() => {
      vi.advanceTimersByTime(1)
    })
    expect(result.current).toBe("updated")
  })

  it("works with non-string values", () => {
    const { result, rerender } = renderHook(
      ({ value }) => useDebouncedValue<number>(value, 100),
      { initialProps: { value: 0 } }
    )

    rerender({ value: 42 })

    act(() => {
      vi.advanceTimersByTime(100)
    })
    expect(result.current).toBe(42)
  })

  it("works with object values", () => {
    const initial = { name: "initial" }
    const updated = { name: "updated" }

    const { result, rerender } = renderHook(
      ({ value }) => useDebouncedValue(value, 100),
      { initialProps: { value: initial } }
    )

    rerender({ value: updated })

    act(() => {
      vi.advanceTimersByTime(100)
    })
    expect(result.current).toBe(updated)
  })
})
