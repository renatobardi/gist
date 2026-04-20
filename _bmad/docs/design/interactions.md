# Interaction Patterns

This document defines common interaction behaviors for Knowledge Vault.

## 1. Form Behavior

### Validation Timing
- **Inline:** Validate on `blur` for existing fields.
- **Submission:** Full validation on `submit`.
- **Real-time:** Clear error state as soon as the user starts typing in a field with an error.

### Feedback
- **Success:** Toast message or redirect to a success page.
- **Error:** Inline error messages (Component: Form Field) and a summary error toast if multiple fields fail.

## 2. Loading States

### Page Transitions
- **Skeleton Screens:** Use for Library and Book Detail pages during initial SSR/Hydration.
- **Progress Bar:** Thin BMW Blue bar at the top of the viewport during navigation.

### Component Loading
- **Status Badges:** Use a pulsing animation for `Processing` (US-17).
- **Graph:** Show a "Loading Graph..." overlay with a spinner until Cytoscape.js is initialized.

## 3. Real-time Updates (WebSockets)

- **Status Badges:** Must update without page refresh (FR-28).
- **Animation:** Use a subtle "flash" effect (yellow/green background highlight) when a book transitions to `Done`.

## 4. Graph Interactions

- **Dragging Nodes:** Node position is session-local. No persistence in v1 (ADR-03).
- **Tooltips:** Appear on hover with 200ms delay. Disappear on mouse leave.
- **Side Panel:** Slides in from the right. Focus moves to the panel for accessibility.

## 5. Feedback Patterns

- **Toasts:** Use for non-critical notifications (e.g., "Book submitted", "API Key copied").
- **Modals:** Use for critical confirmation actions (e.g., "Revoke API Key").
- **Empty States:** Clear messaging and a primary CTA when no books or concepts exist.
