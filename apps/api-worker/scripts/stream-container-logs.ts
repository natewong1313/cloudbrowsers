const CONTAINER_PATTERN = /BrowserContainer/;
const ID_PREFIX_LENGTH = 8;
const TAIL_LINES = 100;

// ANSI colors
const COLORS = {
  reset: "\x1b[0m",
  red: "\x1b[31m",
  dim: "\x1b[2m",
  cyan: "\x1b[36m",
  green: "\x1b[32m",
};

interface DockerEvent {
  status: string;
  id: string;
  from?: string;
  Type: string;
  Action: string;
  Actor: {
    ID: string;
    Attributes: {
      name: string;
      [key: string]: string;
    };
  };
  time: number;
  timeNano: number;
}

interface DockerContainer {
  id: string;
  name: string;
}

// Track active log streamers
const activeStreamers = new Map<string, any>();

/**
 * Get shortened container ID for display
 */
function getShortId(containerId: string): string {
  return containerId.substring(0, ID_PREFIX_LENGTH);
}

/**
 * Format a log line with container ID prefix
 */
function formatLine(
  shortId: string,
  line: string,
  isStderr: boolean = false,
): string {
  const color = isStderr ? COLORS.red : COLORS.reset;
  return `${COLORS.cyan}[${shortId}]${COLORS.reset} ${color}${line}${COLORS.reset}`;
}

/**
 * Attach to container logs and stream them to stdout
 */
async function attachToLogs(
  containerId: string,
  containerName: string,
): Promise<void> {
  const shortId = getShortId(containerId);

  // Don't attach if already streaming
  if (activeStreamers.has(containerId)) {
    return;
  }

  console.log(
    formatLine(
      shortId,
      `${COLORS.green}📦 Attached to ${containerName}${COLORS.reset}`,
      false,
    ),
  );

  try {
    const proc = Bun.spawn(
      ["docker", "logs", "-f", "--tail", TAIL_LINES.toString(), containerId],
      {
        stdout: "pipe",
        stderr: "pipe",
      },
    );

    activeStreamers.set(containerId, proc);

    // Stream stdout
    (async () => {
      const reader = proc.stdout.getReader();
      const decoder = new TextDecoder();

      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          const text = decoder.decode(value, { stream: true });
          const lines = text.split("\n");

          for (const line of lines) {
            if (line.trim()) {
              console.log(formatLine(shortId, line, false));
            }
          }
        }
      } catch (err) {
        // Stream closed, ignore
      }
    })();

    // Stream stderr (in red)
    (async () => {
      const reader = proc.stderr.getReader();
      const decoder = new TextDecoder();

      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          const text = decoder.decode(value, { stream: true });
          const lines = text.split("\n");

          for (const line of lines) {
            if (line.trim()) {
              console.log(formatLine(shortId, line, true));
            }
          }
        }
      } catch (err) {
        // Stream closed, ignore
      }
    })();

    // Wait for process to exit
    await proc.exited;
  } catch (err) {
    console.error(
      `${COLORS.red}Error attaching to container ${shortId}: ${err}${COLORS.reset}`,
    );
  }
}

/**
 * Detach from container logs
 */
function detachFromLogs(containerId: string): void {
  const proc = activeStreamers.get(containerId);
  if (proc) {
    proc.kill();
    activeStreamers.delete(containerId);
    const shortId = getShortId(containerId);
    console.log(
      formatLine(
        shortId,
        `${COLORS.dim}📦 Container stopped${COLORS.reset}`,
        false,
      ),
    );
  }
}

/**
 * Get existing running containers that match the pattern
 */
async function getExistingContainers(): Promise<DockerContainer[]> {
  try {
    const proc = Bun.spawn(["docker", "ps", "--format", "{{json .}}"], {
      stdout: "pipe",
    });

    const text = await new Response(proc.stdout).text();
    const lines = text
      .trim()
      .split("\n")
      .filter((line) => line.trim());

    const containers: DockerContainer[] = [];

    for (const line of lines) {
      try {
        const container = JSON.parse(line);
        const name = container.Names || "";

        if (CONTAINER_PATTERN.test(name)) {
          containers.push({
            id: container.ID,
            name: name,
          });
        }
      } catch (err) {
        // Skip invalid JSON lines
      }
    }

    return containers;
  } catch (err) {
    console.error(
      `${COLORS.red}Error fetching existing containers: ${err}${COLORS.reset}`,
    );
    return [];
  }
}

/**
 * Watch Docker events for container starts/stops
 */
async function watchDockerEvents(): Promise<void> {
  const proc = Bun.spawn(
    [
      "docker",
      "events",
      "--filter",
      "type=container",
      "--format",
      "{{json .}}",
    ],
    {
      stdout: "pipe",
    },
  );

  const reader = proc.stdout.getReader();
  const decoder = new TextDecoder();

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const text = decoder.decode(value, { stream: true });
      const lines = text.split("\n");

      for (const line of lines) {
        if (!line.trim()) continue;

        try {
          const event: DockerEvent = JSON.parse(line);
          const containerId = event.Actor.ID;
          const containerName = event.Actor.Attributes.name;

          // Check if container name matches our pattern
          if (!CONTAINER_PATTERN.test(containerName)) {
            continue;
          }

          if (event.Action === "start") {
            // Attach to new container
            attachToLogs(containerId, containerName);
          } else if (event.Action === "die" || event.Action === "stop") {
            // Detach from stopped container
            detachFromLogs(containerId);
          }
        } catch (err) {
          // Skip invalid JSON
        }
      }
    }
  } catch (err) {
    console.error(
      `${COLORS.red}Error watching Docker events: ${err}${COLORS.reset}`,
    );
  }
}

/**
 * Cleanup function to kill all active streamers
 */
function cleanup(): void {
  console.log(`\n${COLORS.dim}Shutting down log streamers...${COLORS.reset}`);

  for (const [, proc] of activeStreamers.entries()) {
    try {
      proc.kill();
    } catch (err) {
      // Ignore errors during cleanup
    }
  }

  activeStreamers.clear();
  process.exit(0);
}

/**
 * Main entry point
 */
async function main(): Promise<void> {
  console.log(`${COLORS.green}🐳 Docker log streamer started${COLORS.reset}`);
  console.log(
    `${COLORS.dim}   Watching for containers matching: ${CONTAINER_PATTERN}${COLORS.reset}\n`,
  );

  // Register signal handlers
  process.on("SIGINT", cleanup);
  process.on("SIGTERM", cleanup);

  // Attach to existing containers
  const existingContainers = await getExistingContainers();
  for (const container of existingContainers) {
    await attachToLogs(container.id, container.name);
  }

  // Start watching for new containers
  await watchDockerEvents();
}

// Run the script
main().catch((err) => {
  console.error(`${COLORS.red}Fatal error: ${err}${COLORS.reset}`);
  process.exit(1);
});
