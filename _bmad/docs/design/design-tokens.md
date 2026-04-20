# Design Tokens

This document defines the framework-agnostic design tokens for Knowledge Vault, adhering to the BMW design system constraints.

## 1. Color Palette

### Brand & Interactive
| Token | Value | Description | Contrast (on #FFF) |
|-------|-------|-------------|-------------------|
| `color-brand-blue` | `#1c69d4` | BMW Blue. Used for interactive elements only. | 4.53:1 (Pass AA) |
| `color-brand-blue-hover` | `#0653b6` | BMW Focus Blue. Used for hover/focus states. | 6.88:1 (Pass AA) |

### Neutrals
| Token | Value | Description |
|-------|-------|-------------|
| `color-text-primary` | `#262626` | Near-black. Main body text. |
| `color-text-secondary` | `#666666` | Secondary text, labels, metadata. |
| `color-bg-main` | `#ffffff` | Main application background. |
| `color-bg-dark` | `#1a1a1a` | Hero sections, dark headers. |
| `color-border` | `#e5e5e5` | Subtle borders for cards and dividers. |

### Semantic
| Token | Value | Description |
|-------|-------|-------------|
| `color-success` | `#28a745` | Success status, active badges. |
| `color-error` | `#dc3545` | Error messages, failed status. |
| `color-warning` | `#ffc107` | Warnings, pending states. |
| `color-info` | `#17a2b8` | Informational badges. |

## 2. Typography

| Token | Value | Description |
|-------|-------|-------------|
| `font-family-sans` | `Inter, Helvetica, Arial, sans-serif` | Primary font stack. |
| `line-height-tight` | `1.15` | Used for headings. |
| `line-height-normal` | `1.30` | Used for body copy. |

### Font Sizes
| Token | Value | Usage |
|-------|-------|-------|
| `font-size-xs` | `12px` | Metadata, small captions. |
| `font-size-sm` | `14px` | Small text, secondary UI elements. |
| `font-size-base` | `16px` | Default body text. |
| `font-size-lg` | `20px` | Subheadings, card titles. |
| `font-size-xl` | `24px` | Section titles. |
| `font-size-2xl` | `32px` | Page titles (H1). |

## 3. Spacing

Base unit: `4px`

| Token | Value | Description |
|-------|-------|-------------|
| `space-1` | `4px` | Tiny adjustments. |
| `space-2` | `8px` | Small gaps, internal padding. |
| `space-4` | `16px` | Standard padding, component spacing. |
| `space-6` | `24px` | Large gaps between sections. |
| `space-8` | `32px` | Container margins. |
| `space-12` | `48px` | Hero section padding. |

## 4. Layout & Grid

| Token | Value | Description |
|-------|-------|-------------|
| `max-width-container` | `1200px` | Content limit for desktop. |
| `grid-gutter` | `24px` | Space between columns. |
| `breakpoint-md` | `768px` | Tablet/Mobile transition. |
| `breakpoint-lg` | `1024px` | Desktop transition. |

## 5. Components & Borders

| Token | Value | Description |
|-------|-------|-------------|
| `border-radius-none` | `0px` | Sharp corners everywhere. |
| `border-width-thin` | `1px` | Subtle borders. |
| `border-width-thick` | `3px` | Focus rings and primary accents. |
| `shadow-none` | `none` | Clean, flat design style. |
