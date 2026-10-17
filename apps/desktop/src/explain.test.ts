import { describe, expect, it } from "vitest";
import { explainEvidence } from "./AnalysisPanel";
import type { Hypothesis } from "./api";
import { messages } from "./i18n";

const base: Hypothesis = {
  tonic_pc: 2,
  kind: "aeolian",
  score: 0.8,
  probability: 0.57,
  evidence: {
    tonic_share: 0.38,
    fifth_share: 0.2,
    coverage: 0.96,
    decisive: { pc_for: 10, share_for: 0.16, pc_against: 11, share_against: 0.1, rival_tonic_pc: 2, rival_kind: "dorian" },
    off_scale: [{ pc: 6, share: 0.03 }],
  },
};

describe("explainEvidence (F3.1)", () => {
  it("words tonic share, the decisive note with its ratio, coverage and off-scale notes", () => {
    const s = explainEvidence(base, messages.en);
    expect(s).toBe("D holds 38 % of the time; A♯ is 60 % more frequent than B (vs D Dorian); 96 % of the sound is in the scale; outside the scale: F♯ 3 %");
    expect(explainEvidence(base, messages.bg)).toContain("Ре се задържа 38 %");
  });
  it("says 'absent' when the rival's note never sounds and omits a losing decisive", () => {
    const absent = { ...base, evidence: { ...base.evidence!, decisive: { ...base.evidence!.decisive!, share_against: 0 }, off_scale: [] } };
    expect(explainEvidence(absent, messages.en)).toBe("D holds 38 % of the time; A♯ occurs, B does not (vs D Dorian); 96 % of the sound is in the scale");
    const losing = { ...base, evidence: { ...base.evidence!, decisive: { ...base.evidence!.decisive!, share_for: 0.05 }, off_scale: [] } };
    expect(explainEvidence(losing, messages.en)).toBe("D holds 38 % of the time; 96 % of the sound is in the scale");
    expect(explainEvidence({ ...base, evidence: null }, messages.en)).toBe("");
  });
});
