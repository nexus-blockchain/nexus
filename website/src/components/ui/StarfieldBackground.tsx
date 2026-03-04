"use client";

import { useMemo } from "react";

function generateStars(count: number, maxX: number, maxY: number) {
  const shadows: string[] = [];
  for (let i = 0; i < count; i++) {
    const x = Math.floor(Math.random() * maxX);
    const y = Math.floor(Math.random() * maxY);
    shadows.push(`${x}px ${y}px #fff`);
  }
  return shadows.join(", ");
}

export function StarfieldBackground() {
  const layer1 = useMemo(() => generateStars(600, 2000, 2000), []);
  const layer2 = useMemo(() => generateStars(200, 2000, 2000), []);
  const layer3 = useMemo(() => generateStars(80, 2000, 2000), []);

  return (
    <div className="starfield-container" aria-hidden="true">
      {/* Three parallax star layers */}
      <div className="stars-layer stars-small" style={{ boxShadow: layer1 }} />
      <div className="stars-layer stars-medium" style={{ boxShadow: layer2 }} />
      <div className="stars-layer stars-large" style={{ boxShadow: layer3 }} />

      {/* Shooting stars */}
      <div className="shooting-star shooting-star-1" />
      <div className="shooting-star shooting-star-2" />
      <div className="shooting-star shooting-star-3" />
      <div className="shooting-star shooting-star-4" />
    </div>
  );
}
