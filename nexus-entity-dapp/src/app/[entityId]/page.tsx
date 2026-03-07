import { DashboardPage } from './dashboard-client';

// Required for `output: "export"` — entity IDs are dynamic/runtime,
// so we return an empty array and rely on client-side rendering.
export function generateStaticParams() {
  return [];
}

export default function Page() {
  return <DashboardPage />;
}
