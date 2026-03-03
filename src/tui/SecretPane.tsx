import React from "react";
import { Box, Text } from "ink";
import type { PlaintextSecret } from "../types";

export const SecretPane = ({
  secrets,
  selected,
  focused,
  showValues,
}: {
  secrets: PlaintextSecret[];
  selected: number;
  focused: boolean;
  showValues: boolean;
}) => {
  const color = focused ? "cyan" : "gray";
  return (
    <Box flexDirection="column" flexGrow={1} borderStyle="round" borderColor={color}>
      <Box>
        <Text bold color={color}> {"Name".padEnd(22)}</Text>
        <Text bold color={color}>{"Value".padEnd(24)}</Text>
        <Text bold color={color}>Tags</Text>
      </Box>
      {secrets.length === 0 ? (
        <Text dimColor>  (none)</Text>
      ) : (
        secrets.map((s, i) => {
          const isSelected = i === selected && focused;
          const value = showValues ? s.value : "••••••••";
          return (
            <Box key={s.id}>
              <Text color={isSelected ? "cyan" : "white"}>
                {isSelected ? "› " : "  "}
                {s.name.padEnd(20)}
              </Text>
              <Text color={isSelected ? "cyan" : "gray"}>{value.padEnd(24)}</Text>
              <Text color={isSelected ? "cyan" : "gray"} dimColor>
                {s.tags.join(", ")}
              </Text>
            </Box>
          );
        })
      )}
    </Box>
  );
};
