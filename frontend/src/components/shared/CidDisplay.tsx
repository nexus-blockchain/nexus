"use client";

import { useState, useEffect } from "react";
import { ipfsUrl, detectCidType, fetchCidContent, type CidContentType } from "@/lib/ipfs";
import { ExternalLink, Image as ImageIcon, FileJson, FileText, File, Loader2, Copy } from "lucide-react";

interface CidDisplayProps {
  cid: string | null | undefined;
  label?: string;
  showPreview?: boolean;
  maxPreviewHeight?: number;
  className?: string;
}

export function CidDisplay({ cid, label, showPreview = true, maxPreviewHeight = 200, className }: CidDisplayProps) {
  const [contentType, setContentType] = useState<CidContentType>("unknown");
  const [textContent, setTextContent] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!cid || !showPreview) return;
    let cancelled = false;

    const detect = async () => {
      setIsLoading(true);
      try {
        const type = await detectCidType(cid);
        if (cancelled) return;
        setContentType(type);
        if (type === "json" || type === "text") {
          const content = await fetchCidContent(cid);
          if (!cancelled) setTextContent(content);
        }
      } catch {
        if (!cancelled) setContentType("unknown");
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    };

    detect();
    return () => { cancelled = true; };
  }, [cid, showPreview]);

  if (!cid) {
    return (
      <span className={`text-sm text-muted-foreground italic ${className || ""}`}>
        {label ? `${label}: ` : ""}Not set
      </span>
    );
  }

  const handleCopy = async () => {
    await navigator.clipboard.writeText(cid);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const typeIcon = () => {
    switch (contentType) {
      case "image": return <ImageIcon className="h-3.5 w-3.5" />;
      case "json": return <FileJson className="h-3.5 w-3.5" />;
      case "text": return <FileText className="h-3.5 w-3.5" />;
      default: return <File className="h-3.5 w-3.5" />;
    }
  };

  const shortCid = cid.length > 20 ? `${cid.slice(0, 8)}...${cid.slice(-8)}` : cid;

  return (
    <div className={`space-y-2 ${className || ""}`}>
      {label && <p className="text-sm font-medium">{label}</p>}
      <div className="flex items-center gap-2">
        {!isLoading && typeIcon()}
        {isLoading && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
        <span className="font-mono text-sm" title={cid}>{shortCid}</span>
        <button
          onClick={handleCopy}
          className="text-muted-foreground hover:text-foreground transition-colors"
          title={copied ? "Copied!" : "Copy CID"}
        >
          <Copy className="h-3.5 w-3.5" />
        </button>
        <a
          href={ipfsUrl(cid)}
          target="_blank"
          rel="noopener noreferrer"
          className="text-muted-foreground hover:text-foreground transition-colors"
          title="Open in IPFS Gateway"
        >
          <ExternalLink className="h-3.5 w-3.5" />
        </a>
      </div>

      {showPreview && !isLoading && contentType === "image" && (
        <div className="rounded-md border overflow-hidden" style={{ maxHeight: maxPreviewHeight }}>
          <img
            src={ipfsUrl(cid)}
            alt="IPFS content"
            className="w-full h-auto object-contain"
            style={{ maxHeight: maxPreviewHeight }}
            loading="lazy"
          />
        </div>
      )}

      {showPreview && !isLoading && (contentType === "json" || contentType === "text") && textContent && (
        <pre className="rounded-md border bg-muted p-3 text-xs overflow-auto" style={{ maxHeight: maxPreviewHeight }}>
          {contentType === "json" ? JSON.stringify(JSON.parse(textContent), null, 2) : textContent}
        </pre>
      )}
    </div>
  );
}
