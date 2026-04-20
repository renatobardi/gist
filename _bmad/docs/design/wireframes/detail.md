# Wireframe: Book Detail

Detailed insights extracted for a specific work.

## Layout

```mermaid
graph TD
    subgraph Page ["/works/{id}"]
        Header["Header (Standard)"]
        subgraph Meta ["Book Meta"]
            Title["# Book Title"]
            Author["## Author Name"]
        end
        subgraph Insight ["Insight Section (Gray Background)"]
            SummaryTitle["### Summary"]
            SummaryText["2-4 sentences of executive summary."]
        end
        subgraph KeyPoints ["Key Points"]
            KPTitle["### Key Points"]
            KPList["* Point 1\n* Point 2\n* Point 3"]
        end
        subgraph Concepts ["Extracted Concepts"]
            CTitle["### Concepts"]
            CGrid["Grid of Concept Cards: Name, Domain, Weight"]
        end
    end
```

## Styling

### Summary Box
- **Background:** `#f9f9f9`.
- **Border-left:** 4px solid `#1c69d4`.
- **Padding:** 24px (`space-6`).

### Concept Cards (Mini)
- **Border:** 1px solid `#e5e5e5`.
- **Padding:** 12px.
- **Content:**
    - Name (Inter Bold)
    - Domain (Small text, colored)
    - Relevance Bar (Horizontal bar showing 0.0 to 1.0 strength)
