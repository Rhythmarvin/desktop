# AI Chat IDE - Design Document

## Goals

The target is an IDE-like AI Chat application mimicking the aesthetics of VSCode and Cursor.

## UI / UX Guidelines

- **Theming**: Light theme only (for now).
- **Color Palette**:
  - Primary Accent: `oklch(0.71 0.18 38.65)` (used for highlights and button hovers)
  - Button Base: `oklch(0.66 0.18 38.65)`
  - Backgrounds: Whites (`#ffffff`) and slight grays (`#f3f3f3`, `#fafafa`) for depth.
  - Text: Dark grays (`#24292f`, `#57606a`) for hierarchy.
  - Borders: Subtle dividing lines (`#d0d7de`, `#e5e7eb`).

## Technical Stack

- **Framework**: React + Vite
- **UI Primitives**: Radix UI for accessible base components (ScrollArea, Avatar, etc).
- **Icons**: Lucide for crisp SVG components.
