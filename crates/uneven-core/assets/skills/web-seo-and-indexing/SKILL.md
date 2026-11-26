---
name: web-seo-and-indexing
description: >-
  Use this skill when adding or auditing web page SEO, Google Search snippet
  character limits, SSR metadata, Open Graph, JSON-LD structured data,
  llms.txt, robots.txt, or sitemap.xml.
---

# Web SEO, Structured Data & Indexing

Framework-agnostic standards for search engine visibility, AI discovery (`llms.txt`), and private route protection.

## 1. Google SERP Character Limits

- **Page `<title>`:** Keep between **50 and 60 characters** (maximum `60`) to prevent truncation in Google search results.
- **Meta `<meta name="description">`:** Keep between **120 and 155 characters** (maximum `160`) so the full snippet displays cleanly.

## 2. Public Page Metadata & SSR Rendering

- **Server-rendered metadata for dynamic pages:** For dynamic routes (such as blog posts or product pages), fetch data on the server prior to rendering (e.g., `use_server_future` in SSR) so the initial HTML response returned to crawlers already contains the page-specific `<title>`, `<meta name="description">`, canonical URL, and social tags.
- **Required head tags on every public page:**
  - `<title>` and `<meta name="description">`
  - Open Graph: `og:type` (`website` or `article`), `og:site_name`, `og:title`, `og:description`, `og:url`, `og:image`, `og:locale`
  - Twitter Card: `twitter:card` (`summary_large_image`), `twitter:title`, `twitter:description`, `twitter:image`
  - Canonical link: `<link rel="canonical" href="...">`

## 3. Structured Data (`JSON-LD` / Schema.org)

- Embed `<script type="application/ld+json">` on public pages using Schema.org types matching the page:
  - **Articles / Blog posts:** `BlogPosting` or `Article` (`headline`, `description`, `image`, `url`, `mainEntityOfPage`, `publisher`).
  - **Landing / Product pages:** `Organization`, `WebSite`, or `SoftwareApplication` with consistent public metadata.

## 4. AI Discovery (`llms.txt`)

- Maintain `public/llms.txt` in structured Markdown:
  - `# Organization / Project Name`
  - `> One-sentence summary`
  - Public contact and official profile links
  - Categorized sections (`## Services`, `## Products`, `## Content / Docs`) with concise descriptions and public URLs only.
- Serve `/llms.txt` explicitly with `Content-Type: text/plain; charset=utf-8`.

## 5. Sensitive Routes (`noindex`) & `sitemap.xml`

- **Private / sensitive routes** (login, registration, dashboards, admin panels, internal user/client routes) must include `<meta name="robots" content="noindex, nofollow">` and must **never** be listed in `robots.txt`, `sitemap.xml`, `llms.txt`, or JSON-LD.
- **Public routes:** Include all indexable public pages in `sitemap.xml` (served with `Content-Type: application/xml; charset=utf-8`), and reference the sitemap URL in `public/robots.txt`.
