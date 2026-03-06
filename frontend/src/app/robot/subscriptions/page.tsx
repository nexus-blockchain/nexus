"use client";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { StatusBadge } from "@/components/shared/StatusBadge";
import {
  CreditCard,
  ArrowLeft,
  Check,
  Star,
  Zap,
  Crown,
} from "lucide-react";
import Link from "next/link";
import { useTranslations } from "next-intl";

const TIERS = [
  {
    name: "Free",
    price: "0 NEX/mo",
    icon: Star,
    features: ["1 bot", "1 community", "Basic moderation", "100 messages/day"],
    current: false,
  },
  {
    name: "Pro",
    price: "50 NEX/mo",
    icon: Zap,
    features: ["5 bots", "10 communities", "Advanced moderation", "Reputation system", "10,000 messages/day"],
    current: true,
  },
  {
    name: "Enterprise",
    price: "200 NEX/mo",
    icon: Crown,
    features: ["Unlimited bots", "Unlimited communities", "Priority support", "Custom plugins", "Unlimited messages", "Dedicated nodes"],
    current: false,
  },
];

export default function RobotSubscriptionsPage() {
  const t = useTranslations("common");

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href="/robot"><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold tracking-tight flex items-center gap-2">
            <CreditCard className="h-7 w-7" />
            Subscriptions
          </h1>
          <p className="text-muted-foreground">Choose a plan that fits your community needs</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Current Plan</CardTitle>
          <CardDescription>You are currently on the Pro plan</CardDescription>
        </CardHeader>
        <CardContent className="flex items-center gap-4">
          <div className="rounded-lg bg-primary/10 p-3">
            <Zap className="h-6 w-6 text-primary" />
          </div>
          <div>
            <p className="text-lg font-semibold">Pro</p>
            <p className="text-sm text-muted-foreground">Renews on February 15, 2025</p>
          </div>
          <Badge className="ml-auto" variant="default">Active</Badge>
        </CardContent>
      </Card>

      <div className="grid gap-6 md:grid-cols-3">
        {TIERS.map((tier) => {
          const Icon = tier.icon;
          return (
            <Card key={tier.name} className={tier.current ? "border-primary shadow-md" : ""}>
              <CardHeader>
                <div className="flex items-center gap-2">
                  <Icon className={`h-5 w-5 ${tier.current ? "text-primary" : "text-muted-foreground"}`} />
                  <CardTitle>{tier.name}</CardTitle>
                </div>
                <p className="text-2xl font-bold">{tier.price}</p>
              </CardHeader>
              <CardContent className="space-y-4">
                <ul className="space-y-2">
                  {tier.features.map((feature) => (
                    <li key={feature} className="flex items-center gap-2 text-sm">
                      <Check className="h-4 w-4 text-green-500 shrink-0" />
                      {feature}
                    </li>
                  ))}
                </ul>
                <Button
                  variant={tier.current ? "outline" : "default"}
                  className="w-full"
                  disabled={tier.current}
                >
                  {tier.current ? "Current Plan" : "Upgrade"}
                </Button>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
