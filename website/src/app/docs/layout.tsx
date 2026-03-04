import { getSidebarData } from "@/lib/docs";
import { DocsShell } from "@/components/docs/DocsShell";

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  const sidebarData = getSidebarData("zh");

  return <DocsShell sidebarData={sidebarData}>{children}</DocsShell>;
}
