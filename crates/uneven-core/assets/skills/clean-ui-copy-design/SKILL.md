---
name: clean-ui-copy-design
description: >-
  Use this skill when designing or reviewing UI layouts, user-facing copy,
  icons, media accessibility, or i18n-ready text separation across any web or
  desktop frontend.
---

# Clean UI, Copy & Visual Design

Framework-agnostic standards for minimalist interfaces, consistent Iconify icons, accessible media, and translation-ready copy.

## 1. Visual Hierarchy & Layout

- **Clean, breathable layout:** Use generous whitespace, crisp `1px` borders, subtle elevation, high contrast, and **one primary accent color** without visual clutter.
- **Eyebrow labels instead of tag pills:** Never use decorative pill-shaped tag badges (`rounded-full` tag stacks). Use a clean uppercase eyebrow label with wide letter-spacing above headings when categorization is needed.
- **Clear hero CTAs:** Keep hero actions focused (one solid primary CTA + one bordered secondary CTA) paired with a single short supporting line.
- **Icons via Iconify CSS:** Never use generic/decorative emojis or ad-hoc inline SVGs. Use **Iconify CSS classes** (e.g., `heroicons--*`, `lucide--*`, `devicon-plain--*` from `iconify.css`) styled with `currentColor` and size utility classes.

## 2. Copy & Localization Discipline

- **Separate UI text for translation (i18n-ready):** Keep user-facing strings decoupled from UI layout/components (in dedicated locale dictionaries, copy modules, or message maps) so the interface can be translated cleanly without hunting through markup.
- **Zero em-dashes or en-dashes:** Never use em-dashes (`—`) or en-dashes (`–`) in user-facing copy; use commas, periods, or colons instead.
- **Zero internal tech jargon:** Do not expose internal frameworks, libraries, languages, or implementation details to end users (unless the page is explicitly for a developer tool).
- **Zero redundant or self-explanatory copy:** Keep labels, status indicators, and descriptions concise and purposeful—remove filler text and obvious explanations.

## 3. Media Accessibility

- **Images (`<img>`):** Always provide a clear, descriptive `alt` attribute (or `alt=""` strictly for purely decorative images).
- **Videos (`<video>`):** Always include accessible metadata/labels (`aria-label` or `title`), fallback text, and captions/subtitles (`<track>`) or a short text description when conveying information.
