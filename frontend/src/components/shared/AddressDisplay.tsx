"use client";

import { shortenAddress } from "@/lib/utils";
import { Copy, ExternalLink } from "lucide-react";
import { useState } from "react";

interface AddressDisplayProps {
  address: string;
  chars?: number;
  showCopy?: boolean;
  className?: string;
}

export function AddressDisplay({ address, chars = 6, showCopy = true, className }: AddressDisplayProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <span className={`inline-flex items-center gap-1 font-mono text-sm ${className || ""}`}>
      <span title={address}>{shortenAddress(address, chars)}</span>
      {showCopy && (
        <button onClick={handleCopy} className="text-muted-foreground hover:text-foreground transition-colors" title={copied ? "Copied!" : "Copy address"}>
          <Copy className="h-3.5 w-3.5" />
        </button>
      )}
    </span>
  );
}
