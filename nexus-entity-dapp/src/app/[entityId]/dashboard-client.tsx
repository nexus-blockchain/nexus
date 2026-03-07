'use client';

import { useEntityContext } from './entity-provider';
import { isFundWarning } from '@/lib/utils/fund-warning';
import { useTranslations } from 'next-intl';
import { AlertTriangle, Wallet, Users, ShoppingCart, Coins, BarChart3, TrendingUp, UserPlus } from 'lucide-react';

/** Default fund warning threshold: 10 NEX (10 * 10^12 planck) */
const FUND_WARNING_THRESHOLD = BigInt('10000000000000');

/** Format bigint balance to human-readable NEX (divide by 10^12) */
function formatNexBalance(balance: bigint): string {
  const divisor = BigInt('1000000000000');
  const whole = balance / divisor;
  const remainder = balance % divisor;
  const decStr = remainder.toString().padStart(12, '0').slice(0, 4);
  const trimmed = decStr.replace(/0+$/, '');
  return trimmed ? `${whole.toLocaleString()}.${trimmed}` : whole.toLocaleString();
}

interface StatCardProps {
  label: string;
  value: string;
  icon: React.ReactNode;
  warning?: boolean;
  subtitle?: string;
}

function StatCard({ label, value, icon, warning, subtitle }: StatCardProps) {
  const t = useTranslations('dashboard');
  return (
    <div
      className={`rounded-lg border p-4 ${
        warning
          ? 'border-amber-300 bg-amber-50 dark:border-amber-700 dark:bg-amber-950'
          : 'border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900'
      }`}
    >
      <div className="flex items-center justify-between">
        <span className="text-sm text-gray-500 dark:text-gray-400">{label}</span>
        <span className={warning ? 'text-amber-500' : 'text-gray-400 dark:text-gray-500'}>{icon}</span>
      </div>
      <p className={`mt-2 text-2xl font-semibold ${warning ? 'text-amber-700 dark:text-amber-400' : 'text-gray-900 dark:text-gray-100'}`}>
        {value}
      </p>
      {subtitle && <p className="mt-1 text-xs text-gray-400 dark:text-gray-500">{subtitle}</p>}
      {warning && (
        <div className="mt-2 flex items-center gap-1 text-xs text-amber-600 dark:text-amber-400">
          <AlertTriangle className="h-3 w-3" />
          <span>{t('lowBalance')}</span>
        </div>
      )}
    </div>
  );
}

interface ChartPlaceholderProps {
  title: string;
  icon: React.ReactNode;
}

function ChartPlaceholder({ title, icon }: ChartPlaceholderProps) {
  const t = useTranslations('dashboard');
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
      <div className="mb-3 flex items-center gap-2">
        <span className="text-gray-400">{icon}</span>
        <h3 className="text-sm font-medium text-gray-700 dark:text-gray-300">{title}</h3>
      </div>
      <div className="flex h-48 items-center justify-center rounded-md bg-gray-50 dark:bg-gray-800">
        <p className="text-sm text-gray-400 dark:text-gray-500">
          {t('chartPlaceholder')}
        </p>
      </div>
    </div>
  );
}

function EntityStatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    Active: 'bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300',
    Suspended: 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300',
    Banned: 'bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300',
    PendingClose: 'bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300',
    Closed: 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300',
    PendingApproval: 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300',
  };
  return (
    <span className={`inline-flex rounded-full px-2.5 py-0.5 text-xs font-medium ${colors[status] ?? 'bg-gray-100 text-gray-700'}`}>
      {status}
    </span>
  );
}

export function DashboardPage() {
  const { entity, isLoading } = useEntityContext();
  const t = useTranslations('dashboard');

  if (isLoading || !entity) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-4 border-gray-200 border-t-blue-600" />
      </div>
    );
  }

  const fundWarning = isFundWarning(entity.fundBalance, FUND_WARNING_THRESHOLD);

  return (
    <div className="mx-auto max-w-6xl space-y-6 p-4 md:p-6">
      {/* Fund warning banner */}
      {fundWarning && (
        <div className="flex items-center gap-3 rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 dark:border-amber-700 dark:bg-amber-950">
          <AlertTriangle className="h-5 w-5 shrink-0 text-amber-500" />
          <p className="text-sm text-amber-700 dark:text-amber-400">
            {t('fundWarning')}
          </p>
        </div>
      )}

      {/* Entity info header */}
      <div className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-900">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h1 className="text-xl font-semibold text-gray-900 dark:text-gray-100">{entity.name}</h1>
            <div className="mt-1 flex flex-wrap items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
              <span>{entity.entityType}</span>
              <span>·</span>
              <EntityStatusBadge status={entity.status} />
              <span>·</span>
              <span>{entity.governanceMode === 'FullDAO' ? 'Full DAO' : t('noGovernance')}</span>
            </div>
          </div>
          {entity.verified && (
            <span className="inline-flex items-center gap-1 rounded-full bg-blue-50 px-2.5 py-0.5 text-xs font-medium text-blue-700 dark:bg-blue-900 dark:text-blue-300">
              ✓ Verified
            </span>
          )}
        </div>
      </div>

      {/* Stat cards */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          label={t('fundBalance')}
          value={`${formatNexBalance(entity.fundBalance)} ${t('nex')}`}
          icon={<Wallet className="h-5 w-5" />}
          warning={fundWarning}
        />
        <StatCard
          label={t('totalMembers')}
          value={t('placeholderValue')}
          icon={<Users className="h-5 w-5" />}
          subtitle={t('dataAvailableWithMemberHooks')}
        />
        <StatCard
          label={t('totalOrders')}
          value={t('placeholderValue')}
          icon={<ShoppingCart className="h-5 w-5" />}
          subtitle={t('dataAvailableWithOrderHooks')}
        />
        <StatCard
          label={t('tokenSymbol')}
          value={t('placeholderValue')}
          icon={<Coins className="h-5 w-5" />}
          subtitle={t('dataAvailableWithTokenHooks')}
        />
      </div>

      {/* Chart placeholders */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2 xl:grid-cols-3">
        <ChartPlaceholder title={t('salesTrend')} icon={<BarChart3 className="h-4 w-4" />} />
        <ChartPlaceholder title={t('memberGrowth')} icon={<UserPlus className="h-4 w-4" />} />
        <ChartPlaceholder title={t('tokenPriceTrend')} icon={<TrendingUp className="h-4 w-4" />} />
      </div>
    </div>
  );
}
