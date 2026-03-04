"use client";

import { use } from "react";
import { useReviews, useReviewActions } from "@/hooks/useReview";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { AddressDisplay } from "@/components/shared/AddressDisplay";
import { ArrowLeft, Star, MessageSquare, Flag } from "lucide-react";
import Link from "next/link";

function StarRating({ rating }: { rating: number }) {
  return (
    <div className="flex items-center gap-0.5">
      {[1, 2, 3, 4, 5].map((star) => (
        <Star
          key={star}
          className={`h-4 w-4 ${star <= rating ? "fill-yellow-400 text-yellow-400" : "text-muted-foreground/30"}`}
        />
      ))}
      <span className="ml-1 text-sm font-medium">{rating}/5</span>
    </div>
  );
}

export default function ReviewsPage({ params }: { params: Promise<{ shopId: string }> }) {
  const { shopId: shopIdStr } = use(params);
  const shopId = Number(shopIdStr);
  const { reviews, isLoading } = useReviews(shopId);
  const actions = useReviewActions();

  const avgRating = reviews.length > 0
    ? (reviews.reduce((sum, r) => sum + r.rating, 0) / reviews.length).toFixed(1)
    : "—";

  const ratingDist = [5, 4, 3, 2, 1].map((r) => ({
    rating: r,
    count: reviews.filter((rev) => rev.rating === r).length,
    pct: reviews.length > 0 ? (reviews.filter((rev) => rev.rating === r).length / reviews.length) * 100 : 0,
  }));

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" asChild>
          <Link href={`/shops/${shopId}`}><ArrowLeft className="h-4 w-4" /></Link>
        </Button>
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Reviews</h1>
          <p className="text-muted-foreground">Customer ratings for Shop #{shopId}</p>
        </div>
      </div>

      <div className="grid gap-6 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Average Rating</CardTitle></CardHeader>
          <CardContent>
            <div className="flex items-center gap-3">
              <span className="text-4xl font-bold">{avgRating}</span>
              <div>
                <StarRating rating={Math.round(Number(avgRating) || 0)} />
                <p className="text-xs text-muted-foreground mt-1">{reviews.length} reviews</p>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card className="md:col-span-2">
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">Rating Distribution</CardTitle></CardHeader>
          <CardContent>
            <div className="space-y-2">
              {ratingDist.map((d) => (
                <div key={d.rating} className="flex items-center gap-3 text-sm">
                  <span className="w-8 text-right">{d.rating} <Star className="inline h-3 w-3 fill-yellow-400 text-yellow-400" /></span>
                  <div className="flex-1 h-2 rounded-full bg-muted overflow-hidden">
                    <div className="h-full rounded-full bg-yellow-400" style={{ width: `${d.pct}%` }} />
                  </div>
                  <span className="w-8 text-muted-foreground">{d.count}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><div className="h-8 w-8 animate-spin rounded-full border-4 border-primary border-t-transparent" /></div>
      ) : reviews.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <MessageSquare className="h-12 w-12 text-muted-foreground/50" />
            <p className="mt-4 text-lg font-medium">No Reviews Yet</p>
            <p className="text-sm text-muted-foreground">Customer reviews will appear here after orders are completed.</p>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-3">
          {reviews.map((review) => (
            <Card key={review.id}>
              <CardContent className="p-4">
                <div className="flex items-start gap-4">
                  <div className="flex-1 space-y-2">
                    <div className="flex items-center gap-3">
                      <StarRating rating={review.rating} />
                      <Badge variant="outline" className="text-xs">{review.status}</Badge>
                      <span className="text-xs text-muted-foreground">Order #{review.orderId}</span>
                    </div>
                    <div className="flex items-center gap-2 text-sm text-muted-foreground">
                      <AddressDisplay address={review.reviewer} chars={4} />
                      <span>&middot; Block #{review.createdAt}</span>
                    </div>
                    {review.contentCid && (
                      <p className="text-sm font-mono text-muted-foreground">{review.contentCid.slice(0, 30)}...</p>
                    )}
                  </div>
                  <div className="flex gap-2">
                    <Button variant="ghost" size="sm" onClick={() => actions.respondToReview(review.id, "")}>
                      <MessageSquare className="mr-1 h-3 w-3" />Reply
                    </Button>
                    <Button variant="ghost" size="sm" onClick={() => actions.flagReview(review.id, "inappropriate")}>
                      <Flag className="mr-1 h-3 w-3" />Flag
                    </Button>
                  </div>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      {actions.txState.status === "finalized" && <p className="text-sm text-green-600">Action completed!</p>}
      {actions.txState.status === "error" && <p className="text-sm text-destructive">{actions.txState.error}</p>}
    </div>
  );
}
