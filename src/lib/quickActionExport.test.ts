import { describe, expect, it } from "vitest";
import { actionExportTarget } from "./quickActionExport";

describe("action export picker", () => {
  it("offers a zip bundle name and filter while files are attached", () => {
    expect(actionExportTarget("docker start", 2)).toEqual({
      defaultPath: "docker start.zip",
      filters: [{ name: "Sprout action bundle", extensions: ["zip"] }],
    });
  });

  it("offers plain JSON while fileless", () => {
    expect(actionExportTarget("docker start", 0)).toEqual({
      defaultPath: "docker start.json",
      filters: [{ name: "Sprout backup", extensions: ["json"] }],
    });
  });
});
