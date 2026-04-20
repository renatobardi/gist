# Component: Form Field

Standard input pattern for setup, login, and book submission.

## Anatomy
- **Label:** Text above the input (Inter Medium, 14px).
- **Input Box:** Rectangular container (0px radius).
- **Helper/Error Text:** Small text below (12px).

## States
- **Default:** 1px solid `#cccccc`.
- **Hover:** 1px solid `color-brand-blue`.
- **Focus:** 3px solid `color-brand-blue-hover` focus ring.
- **Error:** 1px solid `color-error` (`#dc3545`). Text color `#dc3545`.

## Behavior
- **Validation:** Triggered on blur or submit.
- **Dynamic:** Error message appears/disappears reactively.

## Accessibility
- **Label:** Linked via `for` / `id`.
- **Error:** Linked via `aria-describedby`.
- **Requirement:** `aria-required="true"` where applicable.
