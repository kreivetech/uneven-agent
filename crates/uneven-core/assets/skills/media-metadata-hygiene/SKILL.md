---
name: media-metadata-hygiene
description: >-
  Use this skill whenever generating, exporting, or preparing media and
  document files (images, photos, videos, PDFs) for delivery or publication,
  to enforce clean filenames and strip AI/tool metadata.
---

# Media & Document Metadata Hygiene

Standards for clean file naming and metadata sanitization on all deliverable assets.

## File Naming Rules

- Use clean, professional, descriptive filenames reflecting the organization/project (when defined in the local `AGENTS.md`) and the topic (e.g., `Organization_Topic_Subject.pdf`).
- Never leave random hashes, UUIDs, temporary timestamps, or generic placeholder names (`output.png`, `image_1.jpg`, `untitled.pdf`).

## Metadata Sanitization & Attribution

1. **Strip tool and AI traces:** Remove all traces of generation tools, intermediate editors, or AI models from embedded metadata (`EXIF`, `XMP`, `IPTC`, `PDF Info`, `MP4`/video tags).
2. **Apply project-defined ownership:** Populate Author/Creator and Rights/Copyright fields exclusively with the official details defined in the current project's `AGENTS.md` (or project config). Never hardcode unrelated organization names or leave third-party tool signatures.
3. **Verify before delivery:** Inspect the final file's metadata tags to confirm no tool/AI remnants remain.
