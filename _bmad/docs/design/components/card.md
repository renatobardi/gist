# Component: Card

Standard container for books (Library) and concepts (Detail/Graph).

## Anatomy
- **Container:** Rectangular box with border.
- **Header:** Title (Inter Bold).
- **Body:** Content/Metadata.
- **Footer/Badge:** Status or Domain indicators.

## States
- **Default:** 1px solid `#e5e5e5`.
- **Hover:** 1px solid `color-brand-blue`. Background subtly changes to `color-bg-subtle`.
- **Active:** Slight darkening or elevation (if shadow used, but BMW prefers flat).

## Behavior
- **Interactive:** Navigates to a detail page on click.
- **Responsive:** Adapts from grid to list view on mobile (if applicable).

## Accessibility
- **Role:** `link` (interactive cards must be rendered as `<a>` elements, not `<div>`).
- **Keyboard:** Focusable if interactive. `Enter` to activate.
- **Labeling:** Heading level 3 or 4 inside the card for screen readers.
