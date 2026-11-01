import { describe, expect, it } from "vitest";
import { AREAS, AREA_IDS, areaMainScreen, areaOf, areaTabs, NO_PROJECT_SCREENS, SCREEN_TITLE } from "./areas";
import type { Screen } from "./store";

// Every screen the app has (store.ts: Screen). Home and Help are not areas.
const SCREENS: Screen[] = ["home", "play", "compose", "studio", "ambient", "rhythm", "library", "soundscapes", "start", "help", "instrument", "songmap", "samples", "references", "academy", "create", "tuner", "live"];

describe("home areas", () => {
  it("has the five areas of the audit, in order", () => {
    expect(AREA_IDS).toEqual(["play", "background", "song", "learn", "library"]);
  });

  it("places every screen except home and help in exactly one area", () => {
    for (const sc of SCREENS) {
      const owners = AREAS.filter((a) => sc !== "home" && (a.main === sc || a.also.includes(sc)));
      if (sc === "home" || sc === "help") expect(owners).toHaveLength(0);
      else expect(owners, sc).toHaveLength(1);
    }
    expect(areaOf("home")).toBeNull();
    expect(areaOf("tuner")?.id).toBe("play");
    expect(areaOf("ambient")?.id).toBe("background");
    expect(areaOf("live")?.id).toBe("background");
  });

  it("never lists a main screen under 'also' and resolves area ids to their main screen", () => {
    for (const a of AREAS) expect(a.also).not.toContain(a.main);
    expect(areaMainScreen("background")).toBe("compose");
    expect(areaMainScreen("nope")).toBeNull();
  });

  it("gives every screen a title and every area its main tab first", () => {
    for (const sc of SCREENS) if (sc !== "home") expect(SCREEN_TITLE[sc], sc).toBeTruthy();
    for (const a of AREAS) expect(areaTabs(a)[0]).toBe(a.main);
    for (const sc of NO_PROJECT_SCREENS) expect(SCREENS).toContain(sc);
  });
});
