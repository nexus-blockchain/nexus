"use client";

import { useMemo } from "react";
import {
  ResponsiveContainer,
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
} from "recharts";

interface SalesTrendChartProps {
  data: Array<{ label: string; sales: number; orders: number }>;
  height?: number;
}

export function SalesTrendChart({ data, height = 250 }: SalesTrendChartProps) {
  const chartData = useMemo(() => data, [data]);

  if (chartData.length === 0) {
    return (
      <div className="flex items-center justify-center text-muted-foreground text-sm" style={{ height }}>
        No sales data available
      </div>
    );
  }

  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={chartData} margin={{ top: 5, right: 10, left: 0, bottom: 0 }}>
        <defs>
          <linearGradient id="salesGradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="hsl(var(--primary))" stopOpacity={0.3} />
            <stop offset="95%" stopColor="hsl(var(--primary))" stopOpacity={0} />
          </linearGradient>
          <linearGradient id="ordersGradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="hsl(var(--chart-2, 200 70% 50%))" stopOpacity={0.3} />
            <stop offset="95%" stopColor="hsl(var(--chart-2, 200 70% 50%))" stopOpacity={0} />
          </linearGradient>
        </defs>
        <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
        <XAxis
          dataKey="label"
          tick={{ fontSize: 12 }}
          className="text-muted-foreground"
          tickLine={false}
          axisLine={false}
        />
        <YAxis
          tick={{ fontSize: 12 }}
          className="text-muted-foreground"
          tickLine={false}
          axisLine={false}
          width={40}
        />
        <Tooltip
          contentStyle={{
            backgroundColor: "hsl(var(--popover))",
            border: "1px solid hsl(var(--border))",
            borderRadius: "0.5rem",
            fontSize: "0.875rem",
          }}
          labelStyle={{ fontWeight: 600 }}
        />
        <Area
          type="monotone"
          dataKey="sales"
          stroke="hsl(var(--primary))"
          fill="url(#salesGradient)"
          strokeWidth={2}
          name="Sales (NEX)"
        />
        <Area
          type="monotone"
          dataKey="orders"
          stroke="hsl(var(--chart-2, 200 70% 50%))"
          fill="url(#ordersGradient)"
          strokeWidth={2}
          name="Orders"
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
