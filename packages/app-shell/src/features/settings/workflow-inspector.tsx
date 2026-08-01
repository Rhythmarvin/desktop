import { useTranslation } from "react-i18next";
import {
  IconLayoutSidebarRightCollapse,
  IconSettings,
  IconTrash,
} from "@tabler/icons-react";
import {
  Button,
  Input,
  Label,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Textarea,
} from "@ora/ui";
import {
  type WorkflowNodeData,
  type WorkflowCapabilities,
} from "@ora/workflow-mock";
import type { Node } from "@xyflow/react";
import { getNodeMetadata } from "./workflow-node-metadata";

interface WorkflowInspectorProps {
  node: Node<WorkflowNodeData, "workflow"> | null;
  capabilities: WorkflowCapabilities;
  onUpdate: (node: Node<WorkflowNodeData, "workflow">) => void;
  onDelete: (nodeId: string) => void;
  onCloseNode: () => void;
}

/** Right-rail editor for the selected workflow node (definition only). */
export function WorkflowInspector(props: WorkflowInspectorProps) {
  if (props.node === null) {
    return <WorkflowInspectorEmpty />;
  }
  return (
    <WorkflowNodeInspector
      node={props.node}
      capabilities={props.capabilities}
      onUpdate={props.onUpdate}
      onDelete={props.onDelete}
      onClose={props.onCloseNode}
    />
  );
}

/** Shown when the inspector is open but no node is selected. */
function WorkflowInspectorEmpty() {
  const { t } = useTranslation();
  return (
    <aside className="flex min-h-0 flex-1 flex-col border-l border-border bg-background">
      <div className="border-b border-border px-4 py-3">
        <h3 className="text-xs font-semibold">{t("settings.workflow.configuration")}</h3>
        <p className="mt-1 text-[11px] text-muted-foreground">{t("settings.workflow.selectNodeHint")}</p>
      </div>
      <div className="flex flex-1 flex-col items-center justify-center px-6 text-center">
        <span className="mb-3 flex size-10 items-center justify-center rounded-xl bg-muted">
          <IconSettings className="size-5 text-muted-foreground" />
        </span>
        <p className="text-xs font-medium">{t("settings.workflow.noSelection")}</p>
        <p className="mt-1 text-[11px] leading-5 text-muted-foreground">
          {t("settings.workflow.noSelectionHint")}
        </p>
      </div>
    </aside>
  );
}

/** Edits a node in place with visible labels and progressive, kind-specific fields. */
function WorkflowNodeInspector({
  node,
  capabilities,
  onUpdate,
  onDelete,
  onClose,
}: {
  node: Node<WorkflowNodeData, "workflow">;
  capabilities: WorkflowCapabilities;
  onUpdate: (node: Node<WorkflowNodeData, "workflow">) => void;
  onDelete: (nodeId: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const metadata = getNodeMetadata(node.data.kind);
  const nodeType = capabilities.nodeTypes.find((candidate) => candidate.kind === node.data.kind);
  if (nodeType === undefined) {
    throw new Error(`Missing workflow capability for node kind "${node.data.kind}"`);
  }
  const Icon = metadata.icon;
  return (
    <aside className="flex min-h-0 flex-1 flex-col border-l border-border bg-background">
      <div className="flex items-center gap-2.5 border-b border-border px-4 py-3">
        <span className={`flex size-8 items-center justify-center rounded-lg ${metadata.tone}`}>
          <Icon className="size-4" />
        </span>
        <div className="min-w-0 flex-1">
          <h3 className="text-xs font-semibold">{node.data.title}</h3>
          <p className="text-[10px] text-muted-foreground">
            {t("settings.workflow.nodeSuffix", { type: nodeType.label })}
          </p>
        </div>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("settings.workflow.closeConfiguration")}
          onClick={onClose}
        >
          <IconLayoutSidebarRightCollapse />
        </Button>
      </div>
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4">
        <InspectorField label={t("settings.workflow.field.name")} htmlFor="workflow-node-title">
          <Input
            id="workflow-node-title"
            value={node.data.title}
            onChange={(event) => onUpdate({
              ...node,
              data: { ...node.data, title: event.target.value },
            })}
          />
        </InspectorField>
        <InspectorField label={t("settings.workflow.field.description")} htmlFor="workflow-node-description">
          <Input
            id="workflow-node-description"
            value={node.data.description}
            onChange={(event) => onUpdate({
              ...node,
              data: { ...node.data, description: event.target.value },
            })}
          />
        </InspectorField>
        {nodeType.configFields.includes("model") && (
          <InspectorField label={t("settings.workflow.field.model")} htmlFor="workflow-node-model">
            <Select
              value={node.data.model ?? capabilities.defaultModel}
              onValueChange={(model) => {
                if (model !== null) {
                  onUpdate({ ...node, data: { ...node.data, model } });
                }
              }}
            >
              <SelectTrigger id="workflow-node-model" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {capabilities.models.map((model) => (
                  <SelectItem key={model.value} value={model.value}>
                    {model.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </InspectorField>
        )}
        {nodeType.configFields.includes("tool") && (
          <InspectorField label={t("settings.workflow.field.tool")} htmlFor="workflow-node-tool">
            <Select
              value={node.data.tool ?? capabilities.defaultTool}
              onValueChange={(tool) => {
                if (tool !== null) {
                  onUpdate({ ...node, data: { ...node.data, tool } });
                }
              }}
            >
              <SelectTrigger id="workflow-node-tool" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {capabilities.tools.map((tool) => (
                  <SelectItem key={tool.value} value={tool.value}>
                    {tool.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </InspectorField>
        )}
        {nodeType.configFields.includes("condition") && (
          <InspectorField label={t("settings.workflow.field.condition")} htmlFor="workflow-node-condition">
            <Input
              id="workflow-node-condition"
              value={node.data.condition ?? ""}
              onChange={(event) =>
                onUpdate({
                  ...node,
                  data: { ...node.data, condition: event.target.value },
                })
              }
            />
          </InspectorField>
        )}
        {nodeType.configFields.includes("instruction") && (
          <InspectorField label={t("settings.workflow.field.instruction")} htmlFor="workflow-node-instruction">
            <Textarea
              id="workflow-node-instruction"
              className="min-h-32 resize-none text-xs leading-5"
              value={node.data.instruction}
              onChange={(event) =>
                onUpdate({
                  ...node,
                  data: { ...node.data, instruction: event.target.value },
                })
              }
            />
          </InspectorField>
        )}
      </div>
      <div className="border-t border-border p-3">
        <Button
          variant="ghost"
          className="w-full justify-start text-destructive hover:bg-destructive/10 hover:text-destructive"
          onClick={() => onDelete(node.id)}
          disabled={node.data.kind === "start"}
        >
          <IconTrash />
          {t("settings.workflow.deleteNode")}
        </Button>
      </div>
    </aside>
  );
}

/** Keeps field labels visible and consistently spaced for scanning and accessibility. */
function InspectorField({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor={htmlFor} className="text-[11px]">{label}</Label>
      {children}
    </div>
  );
}
