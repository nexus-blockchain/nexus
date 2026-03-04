import { notFound } from "next/navigation";
import { MDXRemote } from "next-mdx-remote/rsc";
import remarkGfm from "remark-gfm";
import rehypeSlug from "rehype-slug";
import rehypeAutolinkHeadings from "rehype-autolink-headings";
import { getDocBySlug, getAdjacentDocs, getAllDocParams } from "@/lib/docs";
import { DocArticleShell } from "@/components/docs/DocArticleShell";
import { mdxComponents } from "@/components/docs/mdx";

interface PageProps {
  params: Promise<{
    category: string;
    slug: string[];
  }>;
}

export function generateStaticParams() {
  return getAllDocParams("zh");
}

export default async function DocPage({ params }: PageProps) {
  const { category, slug } = await params;

  if (category !== "business" && category !== "technical") {
    notFound();
  }

  const doc = getDocBySlug("zh", category, slug);
  if (!doc) notFound();

  const adjacent = getAdjacentDocs("zh", category as "business" | "technical", slug.join("/"));

  return (
    <DocArticleShell
      doc={doc}
      category={category}
      prev={adjacent.prev}
      next={adjacent.next}
    >
      <MDXRemote
        source={doc.content}
        components={mdxComponents}
        options={{
          mdxOptions: {
            remarkPlugins: [remarkGfm],
            rehypePlugins: [rehypeSlug, [rehypeAutolinkHeadings, { behavior: "wrap" }]],
          },
        }}
      />
    </DocArticleShell>
  );
}
