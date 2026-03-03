import React from "react";
import { Box, Text } from "ink";
import type { PlaintextSecret } from "../types";

export const ProjectSecretsView = ({
  projectName,
  secrets,
  selectedIds,
  cursor,
}: {
  projectName: string;
  secrets: PlaintextSecret[];
  selectedIds: Set<string>;
  cursor: number;
}) => (
  <Box flexDirection="column" padding={1}>
    <Box flexDirection="column" borderStyle="round" borderColor="cyan" padding={1}>
      <Text bold color="cyan">Manage Secrets — {projectName}</Text>
      <Box marginTop={1} flexDirection="column">
        {secrets.length === 0 ? (
          <Text dimColor>  No secrets in workspace yet</Text>
        ) : (
          secrets.map((s, i) => {
            const isCursor = i === cursor;
            const checked = selectedIds.has(s.id);
            return (
              <Box key={s.id}>
                <Text color={isCursor ? "cyan" : "white"}>
                  {isCursor ? "› " : "  "}
                  <Text color={checked ? "green" : "gray"}>{checked ? "[x]" : "[ ]"}</Text>
                  {" "}{s.name}
                </Text>
                {s.tags.length > 0 && (
                  <Text dimColor>  {s.tags.join(", ")}</Text>
                )}
              </Box>
            );
          })
        )}
      </Box>
      <Box marginTop={1}>
        <Text dimColor>[Space] toggle  [Enter] save  [Esc] cancel</Text>
      </Box>
    </Box>
  </Box>
);
