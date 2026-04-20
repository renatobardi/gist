# Component: Status Badge

Visual indicator for book processing states.

## Anatomy
- **Label:** Text (Inter Bold, 12px, Uppercase).
- **Container:** Small rectangle with padding.

## States

| Status | Background | Text Color | Icon/Effect |
|--------|------------|------------|-------------|
| **Pending** | `#ffc107` (Warning) | `#262626` | None |
| **Processing** | `#1c69d4` (Blue) | `#ffffff` | Pulsing animation |
| **Done** | `#28a745` (Success) | `#ffffff` | Checkmark (optional) |
| **Failed** | `#dc3545` (Error) | `#ffffff` | Exclamation (optional) |

## Behavior
- **Reactive:** Updates instantly via WebSocket event.
- **Transition:** Subtle fade when status changes.

## Accessibility
- **ARIA:** `aria-live="polite"` when status changes.
- **Role:** `status`.
