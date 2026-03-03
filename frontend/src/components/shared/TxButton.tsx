"use client";

import { Button, type ButtonProps } from "@/components/ui/button";
import { Loader2 } from "lucide-react";

interface TxButtonProps extends ButtonProps {
  txStatus?: string;
  loadingText?: string;
}

export function TxButton({ txStatus, loadingText = "Processing...", children, disabled, ...props }: TxButtonProps) {
  const isLoading = txStatus === "signing" || txStatus === "broadcasting" || txStatus === "inBlock";

  return (
    <Button disabled={disabled || isLoading} {...props}>
      {isLoading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
      {isLoading ? loadingText : children}
    </Button>
  );
}
