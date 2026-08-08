import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { TaskChangesNavigationProvider } from "../diff/task-changes-navigation";
import type { TurnDiffFile } from "../chat/turn-diff-files";
import { RunActFileChanges } from "./run-act-file-changes";

const files: TurnDiffFile[] = [
  {
    path: "/data/worktrees/run-1/src/foo.ts",
    oldText: "before",
    newText: "after",
    additions: 2,
    deletions: 1,
  },
];

describe("RunActFileChanges", () => {
  it("opens the worktree-relative path when a file is clicked", async () => {
    const openFile = vi.fn();
    const user = userEvent.setup();
    render(
      <TaskChangesNavigationProvider onOpenFile={openFile}>
        <RunActFileChanges files={files} />
      </TaskChangesNavigationProvider>,
    );

    // The ACP diff path is absolute; the task Changes panel matches on the
    // worktree-relative path, so the click must open the normalized form.
    await user.click(screen.getByRole("button", { name: /src\/foo\.ts/ }));
    expect(openFile).toHaveBeenCalledWith("src/foo.ts");
  });
});
