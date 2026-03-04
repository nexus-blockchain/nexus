import { HeroSection } from "@/components/home/HeroSection";
import { PainPointsSection } from "@/components/home/PainPointsSection";
import { TokenLoopSection } from "@/components/home/TokenLoopSection";
import { ThreeCoresSection } from "@/components/home/ThreeCoresSection";
import { CTASection } from "@/components/home/CTASection";

export default function HomePage() {
  return (
    <>
      <HeroSection />
      <PainPointsSection />
      <TokenLoopSection />
      <ThreeCoresSection />
      <CTASection />
    </>
  );
}
