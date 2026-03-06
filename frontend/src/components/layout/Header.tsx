"use client";

import { useWallet } from "@/hooks/useWallet";
import { useApi } from "@/hooks/useApi";
import { useWalletStore } from "@/stores/wallet";
import { useEntityStore } from "@/stores/entity";
import { useUserEntities } from "@/hooks/useEntity";
import { shortenAddress, formatBalance } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Wallet, ChevronDown, Loader2, Wifi, WifiOff, Menu } from "lucide-react";
import { useState } from "react";
import { useTranslations } from "next-intl";
import { LanguageSwitcher } from "@/components/shared/LanguageSwitcher";
import { NotificationCenter } from "@/components/shared/NotificationCenter";
import { useUiStore } from "@/stores/ui";

export function Header() {
  const { connect, disconnect, accounts, isConnecting } = useWallet();
  const { address, isConnected, balance } = useWalletStore();
  const { chainInfo, isConnected: apiConnected, isConnecting: apiConnecting } = useApi();
  const { currentEntityId, setCurrentEntityId, userEntities } = useEntityStore();
  const [showEntityDropdown, setShowEntityDropdown] = useState(false);
  const [showAccountDropdown, setShowAccountDropdown] = useState(false);
  const { toggleMobileSidebar } = useUiStore();
  const t = useTranslations("header");

  useUserEntities(address);

  const currentEntity = userEntities.find((e) => e.id === currentEntityId);

  return (
    <header className="flex h-14 items-center justify-between border-b bg-background px-4">
      <div className="flex items-center gap-4">
        <button
          onClick={toggleMobileSidebar}
          className="rounded-md p-1.5 hover:bg-accent md:hidden"
          aria-label="Toggle menu"
        >
          <Menu className="h-5 w-5" />
        </button>
        <div className="relative">
          <button
            onClick={() => setShowEntityDropdown(!showEntityDropdown)}
            className="flex items-center gap-2 rounded-lg border px-3 py-1.5 text-sm hover:bg-accent"
          >
            <span className="font-medium">
              {currentEntity ? currentEntity.name : t("selectEntity")}
            </span>
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          </button>
          {showEntityDropdown && (
            <div className="absolute left-0 top-full z-50 mt-1 w-56 rounded-md border bg-popover p-1 shadow-md">
              {userEntities.length === 0 ? (
                <div className="px-3 py-2 text-sm text-muted-foreground">{t("noEntities")}</div>
              ) : (
                userEntities.map((e) => (
                  <button
                    key={e.id}
                    onClick={() => {
                      setCurrentEntityId(e.id);
                      setShowEntityDropdown(false);
                    }}
                    className={`flex w-full items-center gap-2 rounded-sm px-3 py-2 text-sm hover:bg-accent ${
                      e.id === currentEntityId ? "bg-accent font-medium" : ""
                    }`}
                  >
                    <span>{e.name}</span>
                    <span className="ml-auto text-xs text-muted-foreground">{e.entityType}</span>
                  </button>
                ))
              )}
            </div>
          )}
        </div>
      </div>

      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
          {apiConnecting ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : apiConnected ? (
            <Wifi className="h-3.5 w-3.5 text-green-500" />
          ) : (
            <WifiOff className="h-3.5 w-3.5 text-red-500" />
          )}
          <span>{chainInfo.name || "Disconnected"}</span>
          {apiConnected && <span className="font-mono">#{chainInfo.bestBlock.toLocaleString()}</span>}
        </div>

        <LanguageSwitcher />

        <NotificationCenter />

        {isConnected && address ? (
          <div className="relative">
            <button
              onClick={() => setShowAccountDropdown(!showAccountDropdown)}
              className="flex items-center gap-2 rounded-lg border px-3 py-1.5 text-sm hover:bg-accent"
            >
              <Wallet className="h-4 w-4 text-muted-foreground" />
              <span className="font-mono">{shortenAddress(address)}</span>
              <span className="text-xs text-muted-foreground">{formatBalance(balance)} NEX</span>
            </button>
            {showAccountDropdown && (
              <div className="absolute right-0 top-full z-50 mt-1 w-48 rounded-md border bg-popover p-1 shadow-md">
                {accounts.map((a) => (
                  <button
                    key={a.address}
                    className="flex w-full items-center gap-2 rounded-sm px-3 py-2 text-sm hover:bg-accent"
                  >
                    <span className="font-mono text-xs">{shortenAddress(a.address, 4)}</span>
                    <span className="text-xs text-muted-foreground">{a.name}</span>
                  </button>
                ))}
                <div className="my-1 border-t" />
                <button
                  onClick={() => {
                    disconnect();
                    setShowAccountDropdown(false);
                  }}
                  className="flex w-full items-center gap-2 rounded-sm px-3 py-2 text-sm text-destructive hover:bg-accent"
                >
                  {t("disconnect")}
                </button>
              </div>
            )}
          </div>
        ) : (
          <Button onClick={connect} disabled={isConnecting} size="sm">
            {isConnecting ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <Wallet className="mr-2 h-4 w-4" />}
            {t("connectWallet")}
          </Button>
        )}
      </div>
    </header>
  );
}
