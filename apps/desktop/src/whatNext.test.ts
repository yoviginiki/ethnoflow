import { describe, expect, it } from "vitest";
import { whatNext, type NextSnapshot } from "./whatNext";

const base: NextSnapshot = {
  hasProject: true,
  takes: 2,
  hasScale: true,
  loaded: true,
  layersOn: { drone: true, pad: false, ambient: false, rhythm: false, scapes: false },
  clips: 3,
  exportedMs: null,
  screen: "studio",
};

describe("what next", () => {
  it("walks the home-studio path in order", () => {
    expect(whatNext({ ...base, hasProject: false }).key).toBe("nextCreateProject");
    expect(whatNext({ ...base, takes: 0 }).key).toBe("nextRecord");
    expect(whatNext({ ...base, hasScale: false }).key).toBe("nextScale");
    expect(whatNext({ ...base, layersOn: { drone: false, pad: false, ambient: false, rhythm: false, scapes: false } }).key).toBe("nextBackground");
    expect(whatNext({ ...base, clips: 0 })).toEqual({ key: "nextArrange", screen: "studio" });
    expect(whatNext(base)).toEqual({ key: "nextExport", screen: "studio" });
    expect(whatNext({ ...base, exportedMs: 1 }).key).toBe("nextPlayAlong");
  });
  it("sends the finished song somewhere else than the screen it is on", () => {
    expect(whatNext({ ...base, exportedMs: 1, screen: "play" }).screen).toBe("library");
    expect(whatNext({ ...base, exportedMs: 1, screen: "library" }).screen).toBe("play");
  });
});
