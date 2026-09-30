import { createElement } from "react"
import type { ReactElement } from "react"

export interface MacOSGlassIconButtonProps {
  icon: string
  size?: number
  disabled?: boolean
  accessibilityLabel?: string
  onClick?: () => void
}

/** An AppKit SF Symbol glass button on macOS, with a GPUI layout placeholder. */
export function MacOSGlassIconButton({
  icon,
  size = 32,
  disabled = false,
  accessibilityLabel,
  onClick,
}: MacOSGlassIconButtonProps): ReactElement {
  const props = {
    icon,
    size,
    disabled,
    accessibilityLabel,
    onClick,
  }
  return createElement("macos-glass-icon-button", props)
}
