import { GpuixRenderer } from "@gpuix/native"
import type { EventPayload } from "@gpuix/native"
import {
  App,
  browserRendererAsTest,
  InProcessBackend,
  liveRendererAsTest,
  serveAutomationStdio,
  type LiveAutomationRenderer,
} from "./automation/client.js"
import { createRendererState } from "./renderer-state.js"

const DEFAULT_FRAME_MS = 8
const NATIVE_WAKE_FALLBACK_MS = 50

export interface FrameLoop {
  stop(): void
}

export interface CreateNativeRendererOptions {
  onEvent?: (event: EventPayload) => void
  onError?: (error: unknown) => void
}

export function createNativeRenderer(
  options: CreateNativeRendererOptions = {}
): GpuixRenderer {
  let renderer!: GpuixRenderer
  renderer = new GpuixRenderer((error, event) => {
    if (error) {
      options.onError?.(error)
      return
    }
    try {
      if (createRendererState(renderer).dispatch(event)) options.onEvent?.(event)
    } catch (dispatchError) {
      options.onError?.(dispatchError)
    }
  })
  if (typeof process !== "undefined" && process.stdin && !process.stdin.isTTY) {
    const init = renderer.init.bind(renderer)
    renderer.init = (windowOptions) => {
      init(windowOptions)
      enableAutomation(renderer)
    }
  }
  return renderer
}

export function startFrameLoop(
  renderer: Pick<GpuixRenderer, "requiresTick" | "tick"> &
    Partial<Pick<GpuixRenderer, "setHostWakeCallback">>,
  options: {
    frameMs?: number
    onTerminated?: () => void
    onError?: (error: unknown) => void
  } = {}
): FrameLoop {
  if (!renderer.requiresTick()) return { stop() {} }
  const nativeWake =
    typeof process !== "undefined" && process.platform === "darwin"
      ? renderer.setHostWakeCallback
      : undefined
  const frameMs = options.frameMs ?? (nativeWake ? NATIVE_WAKE_FALLBACK_MS : DEFAULT_FRAME_MS)
  let timer: ReturnType<typeof setTimeout> | undefined
  let stopped = false
  let ticking = false
  const stop = () => {
    stopped = true
    if (timer !== undefined) clearTimeout(timer)
    timer = undefined
    nativeWake?.call(renderer, undefined)
  }
  const loop = () => {
    if (stopped) return
    if (ticking) return
    if (timer !== undefined) clearTimeout(timer)
    timer = undefined
    ticking = true
    const started = performance.now()
    let running = true
    try {
      running = renderer.tick()
    } catch (error) {
      options.onError?.(error)
    } finally {
      ticking = false
    }
    if (running === false) {
      stop()
      options.onTerminated?.()
      return
    }
    timer = setTimeout(loop, Math.max(0, frameMs - (performance.now() - started)))
  }
  nativeWake?.call(renderer, loop)
  loop()
  return { stop }
}

export function enableAutomation(renderer: LiveAutomationRenderer): void {
  serveAutomationStdio(new InProcessBackend(liveRendererAsTest(renderer)))
}

export function installBrowserAutomation(
  renderer: LiveAutomationRenderer,
  existing?: App
): App {
  if (existing) return existing
  return new App(
    new InProcessBackend(browserRendererAsTest(renderer))
  )
}
