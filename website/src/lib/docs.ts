import fs from "fs";
import path from "path";
import matter from "gray-matter";
import readingTime from "reading-time";

const contentDir = path.join(process.cwd(), "content");

export interface DocMeta {
  title: string;
  description: string;
  slug: string;
  category: string;
  section: string;
  order: number;
  icon: string;
  tags: string[];
  lastUpdated: string;
  readingTime: number;
  difficulty?: "beginner" | "intermediate" | "advanced";
}

export interface Doc extends DocMeta {
  content: string;
  headings: Heading[];
}

export interface Heading {
  depth: number;
  text: string;
  id: string;
}

export interface SidebarItem {
  title: string;
  href: string;
  icon?: string;
  badge?: string;
  children?: SidebarItem[];
}

export interface SidebarSection {
  title: string;
  icon: string;
  category: string;
  items: SidebarItem[];
}

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\w\u4e00-\u9fff\s-]/g, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .trim();
}

export function extractHeadings(content: string): Heading[] {
  const headings: Heading[] = [];
  const regex = /^(#{2,3})\s+(.+)$/gm;
  let match;
  while ((match = regex.exec(content)) !== null) {
    headings.push({
      depth: match[1].length,
      text: match[2].trim(),
      id: slugify(match[2].trim()),
    });
  }
  return headings;
}

function getLocaleDir(locale: string): string {
  const dir = path.join(contentDir, locale);
  if (fs.existsSync(dir)) return dir;
  // fallback to en, then zh
  const enDir = path.join(contentDir, "en");
  if (fs.existsSync(enDir)) return enDir;
  return path.join(contentDir, "zh");
}

/**
 * Recursively find all .mdx files under a directory,
 * returning paths relative to the base directory.
 */
function findMdxFiles(dir: string, base: string = dir): string[] {
  if (!fs.existsSync(dir)) return [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...findMdxFiles(fullPath, base));
    } else if (entry.name.endsWith(".mdx")) {
      files.push(path.relative(base, fullPath));
    }
  }
  return files;
}

/**
 * Parse a single MDX file and return metadata + raw content.
 */
function parseDocFile(filePath: string, category: string, slugParts: string[]): Doc {
  const raw = fs.readFileSync(filePath, "utf-8");
  const { data, content } = matter(raw);
  const stats = readingTime(content);
  const headings = extractHeadings(content);

  return {
    title: data.title || slugParts[slugParts.length - 1] || "Untitled",
    description: data.description || "",
    slug: slugParts.join("/"),
    category,
    section: data.section || slugParts[0] || "",
    order: data.order ?? 999,
    icon: data.icon || "file-text",
    tags: data.tags || [],
    lastUpdated: data.lastUpdated || "",
    readingTime: Math.ceil(stats.minutes),
    difficulty: data.difficulty,
    content,
    headings,
  };
}

/**
 * Get all doc metadata for a given locale and category.
 */
export function getDocsList(locale: string, category: "business" | "technical"): DocMeta[] {
  const localeDir = getLocaleDir(locale);
  const catDir = path.join(localeDir, category);
  if (!fs.existsSync(catDir)) return [];

  const files = findMdxFiles(catDir);
  const docs: DocMeta[] = files.map((relPath) => {
    const fullPath = path.join(catDir, relPath);
    const slugParts = relPath.replace(/\.mdx$/, "").split(path.sep);
    const doc = parseDocFile(fullPath, category, slugParts);
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    const { content: _, headings: __, ...meta } = doc;
    return meta;
  });

  return docs.sort((a, b) => a.order - b.order);
}

/**
 * Get a single doc by category and slug parts.
 */
export function getDocBySlug(
  locale: string,
  category: string,
  slugParts: string[]
): Doc | null {
  const localeDir = getLocaleDir(locale);
  const filePath = path.join(localeDir, category, ...slugParts) + ".mdx";
  if (!fs.existsSync(filePath)) return null;
  return parseDocFile(filePath, category, slugParts);
}

/**
 * Get all valid [category, ...slug] param combos for generateStaticParams.
 */
export function getAllDocParams(locale: string): { category: string; slug: string[] }[] {
  const params: { category: string; slug: string[] }[] = [];
  for (const category of ["business", "technical"]) {
    const docs = getDocsList(locale, category as "business" | "technical");
    for (const doc of docs) {
      params.push({ category, slug: doc.slug.split("/") });
    }
  }
  return params;
}

/**
 * Get previous and next docs relative to the current one.
 */
export function getAdjacentDocs(
  locale: string,
  category: "business" | "technical",
  currentSlug: string
): { prev: DocMeta | null; next: DocMeta | null } {
  const docs = getDocsList(locale, category);
  const idx = docs.findIndex((d) => d.slug === currentSlug);
  return {
    prev: idx > 0 ? docs[idx - 1] : null,
    next: idx >= 0 && idx < docs.length - 1 ? docs[idx + 1] : null,
  };
}

/**
 * Build the sidebar navigation structure for both categories.
 */
export function getSidebarData(locale: string): SidebarSection[] {
  const businessDocs = getDocsList(locale, "business");
  const technicalDocs = getDocsList(locale, "technical");

  // Business docs are flat
  const businessItems: SidebarItem[] = businessDocs.map((d) => ({
    title: d.title,
    href: `/docs/business/${d.slug}`,
    icon: d.icon,
  }));

  // Technical docs are grouped by section (first slug segment)
  const techSections = new Map<string, SidebarItem[]>();
  for (const d of technicalDocs) {
    const parts = d.slug.split("/");
    const section = parts.length > 1 ? parts[0] : "_root";
    if (!techSections.has(section)) techSections.set(section, []);
    techSections.get(section)!.push({
      title: d.title,
      href: `/docs/technical/${d.slug}`,
      icon: d.icon,
    });
  }

  const technicalItems: SidebarItem[] = [];
  for (const [section, items] of techSections) {
    if (section === "_root") {
      technicalItems.push(...items);
    } else {
      technicalItems.push({
        title: section,
        href: items[0]?.href || "#",
        children: items,
      });
    }
  }

  return [
    {
      title: "business",
      icon: "trending-up",
      category: "business",
      items: businessItems,
    },
    {
      title: "technical",
      icon: "wrench",
      category: "technical",
      items: technicalItems,
    },
  ];
}
