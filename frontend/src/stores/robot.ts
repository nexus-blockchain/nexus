import { create } from "zustand";

interface RobotState {
  selectedBotIdHash: string | null;
  setSelectedBotIdHash: (hash: string | null) => void;
  selectedCommunityIdHash: string | null;
  setSelectedCommunityIdHash: (hash: string | null) => void;
}

export const useRobotStore = create<RobotState>((set) => ({
  selectedBotIdHash: null,
  setSelectedBotIdHash: (hash) => set({ selectedBotIdHash: hash }),
  selectedCommunityIdHash: null,
  setSelectedCommunityIdHash: (hash) =>
    set({ selectedCommunityIdHash: hash }),
}));
