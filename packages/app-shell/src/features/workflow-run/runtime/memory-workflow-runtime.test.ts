import { describe, expect, it } from "vitest";
import { createMockWorkflow } from "@ora/workflow-mock";
import { createMemoryWorkflowRuntime } from "./memory-workflow-runtime";

describe("createMemoryWorkflowRuntime", () => {
  it("mounts the same definition on multiple projects by reference", async () => {
    const runtime = createMemoryWorkflowRuntime();
    const definition = createMockWorkflow("zh-CN");
    await runtime.host.mount("p1", definition);
    await runtime.host.mount("p2", definition);
    expect(await runtime.host.listMounts("p1")).toEqual([
      expect.objectContaining({ projectId: "p1", definitionId: definition.id }),
    ]);
    expect(await runtime.host.listMounts("p2")).toEqual([
      expect.objectContaining({ projectId: "p2", definitionId: definition.id }),
    ]);
  });

  it("freezes a definition snapshot when creating a run", async () => {
    const runtime = createMemoryWorkflowRuntime();
    const definition = createMockWorkflow("zh-CN");
    await runtime.host.mount("p1", definition);
    const run = await runtime.runs.create({
      projectId: "p1",
      definitionId: definition.id,
      kickoffInput: "review main",
    });
    definition.name = "mutated-library-name";
    const stored = await runtime.runs.get(run.id);
    expect(stored).toEqual(
      expect.objectContaining({
        id: run.id,
        kickoffInput: "review main",
        status: "pending",
        name: run.name,
      }),
    );
    expect(stored?.definitionSnapshot.name).toBe(run.name);
    expect(stored?.definitionSnapshot.name).not.toBe("mutated-library-name");
  });

  it("rejects create when the definition is not mounted on the project", async () => {
    const runtime = createMemoryWorkflowRuntime();
    const definition = createMockWorkflow("zh-CN");
    await runtime.host.mount("p1", definition);
    await expect(
      runtime.runs.create({ projectId: "p2", definitionId: definition.id }),
    ).rejects.toThrow(/not mounted/);
  });

  it("cancels an open run and emits run_finished", async () => {
    const runtime = createMemoryWorkflowRuntime();
    const definition = createMockWorkflow("zh-CN");
    await runtime.host.mount("p1", definition);
    const run = await runtime.runs.create({
      projectId: "p1",
      definitionId: definition.id,
    });
    const events: string[] = [];
    const unsubscribe = runtime.runs.subscribe(run.id, (event) => {
      events.push(event.type);
    });
    await runtime.runs.cancel(run.id);
    unsubscribe();
    expect(events).toEqual(["run_finished"]);
    expect(await runtime.runs.get(run.id)).toEqual(
      expect.objectContaining({ status: "cancelled" }),
    );
  });

  it("upserts a single mount but allows multiple runs on the same project", async () => {
    const runtime = createMemoryWorkflowRuntime();
    const definition = createMockWorkflow("zh-CN");
    await runtime.host.mount("p1", definition);
    definition.description = "updated blob";
    await runtime.host.mount("p1", definition);
    expect(await runtime.host.listMounts("p1")).toHaveLength(1);
    expect(await runtime.host.listMountsByDefinition(definition.id)).toEqual([
      expect.objectContaining({ projectId: "p1", definitionId: definition.id }),
    ]);
    const first = await runtime.runs.create({
      projectId: "p1",
      definitionId: definition.id,
    });
    const second = await runtime.runs.create({
      projectId: "p1",
      definitionId: definition.id,
    });
    expect(first.id).not.toBe(second.id);
    expect(await runtime.runs.list("p1")).toHaveLength(2);
    expect(second.definitionSnapshot.description).toBe("updated blob");
  });

  it("deletes one run without affecting a sibling run", async () => {
    const runtime = createMemoryWorkflowRuntime();
    const definition = createMockWorkflow("zh-CN");
    await runtime.host.mount("p1", definition);
    const first = await runtime.runs.create({
      projectId: "p1",
      definitionId: definition.id,
    });
    const second = await runtime.runs.create({
      projectId: "p1",
      definitionId: definition.id,
    });
    await runtime.runs.delete(first.id);
    expect(await runtime.runs.get(first.id)).toBeNull();
    expect(await runtime.runs.get(second.id)).toEqual(
      expect.objectContaining({ id: second.id }),
    );
  });

  it("renames a run without changing its definition snapshot", async () => {
    const runtime = createMemoryWorkflowRuntime();
    const definition = createMockWorkflow("zh-CN");
    await runtime.host.mount("p1", definition);
    const run = await runtime.runs.create({
      projectId: "p1",
      definitionId: definition.id,
    });
    const renamed = await runtime.runs.rename(run.id, "  审查一轮  ");
    expect(renamed).toEqual(
      expect.objectContaining({
        id: run.id,
        name: "审查一轮",
        definitionSnapshot: run.definitionSnapshot,
      }),
    );
  });
});
