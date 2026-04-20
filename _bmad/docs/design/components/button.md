# Component: Button

Primary action component following the BMW design system.

## Anatomy
- **Label:** Text (Inter Bold, 14px or 16px).
- **Container:** Rectangular box.

## Variants

### Primary
- **Background:** `color-brand-blue` (`#1c69d4`).
- **Text:** White.
- **Border:** None.
- **Radius:** 0px.

### Secondary (Outline)
- **Background:** Transparent.
- **Text:** `color-brand-blue` (`#1c69d4`).
- **Border:** 1px solid `color-brand-blue`.
- **Radius:** 0px.

### Ghost
- **Background:** Transparent.
- **Text:** `color-text-primary`.
- **Border:** None.
- **Hover:** Light gray background (`#f0f0f0`).

## States

- **Hover:** Background changes to `color-brand-blue-hover` (`#0653b6`) for Primary.
- **Active:** Slight darkening or 1px inset shadow.
- **Focus:** 3px solid `color-brand-blue-hover` focus ring (BMW Focus Blue).
- **Disabled:** Background gray (`#cccccc`), cursor: not-allowed.

## Accessibility
- **Role:** `button`.
- **Keyboard:** `Space` or `Enter` to trigger.
- **Contrast:** Always ≥ 4.5:1.
