// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { FileIcon, Folder } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";

/** File types offered when choosing a game's program. The empty one is Linux binaries. */
const EXECUTABLE_EXTENSIONS = ["exe", "app", "sh", "bat", "cmd", ""];

/** The title, program and install folder of a game, shared by adding and editing. */
export function GameFields({
  id,
  title,
  onTitleChange,
  executablePath,
  onExecutableChange,
  installFolder,
  onInstallFolderChange,
}: {
  /** Prefix for the input ids, unique per dialog. */
  id: string;
  title: string;
  onTitleChange: (title: string) => void;
  executablePath: string;
  onExecutableChange: (path: string) => void;
  installFolder: string;
  onInstallFolderChange: (path: string) => void;
}) {
  async function chooseExecutable() {
    const selected = await openFileDialog({
      multiple: false,
      directory: false,
      title: "Choose the game's program",
      filters: [
        { name: "Programs", extensions: EXECUTABLE_EXTENSIONS },
        { name: "All files", extensions: ["*"] },
      ],
    });
    if (typeof selected === "string") onExecutableChange(selected);
  }

  async function chooseFolder() {
    const selected = await openFileDialog({ multiple: false, directory: true, title: "Choose the install folder" });
    if (typeof selected === "string") onInstallFolderChange(selected);
  }

  return (
    <div className="grid gap-4">
      <Field id={`${id}-title`} label="Title">
        <Input
          id={`${id}-title`}
          placeholder="Game title"
          value={title}
          onChange={(event) => onTitleChange(event.target.value)}
        />
      </Field>
      <Field id={`${id}-executable`} label="Program" hint="Vaultime counts time while this program runs.">
        <div className="flex gap-2">
          <Input
            id={`${id}-executable`}
            readOnly
            placeholder="None chosen"
            value={executablePath}
            className="flex-1 font-mono text-xs"
          />
          <Button variant="outline" size="icon" aria-label="Choose the program" onClick={chooseExecutable}>
            <FileIcon className="size-4" />
          </Button>
        </div>
      </Field>
      <Field id={`${id}-folder`} label="Install folder" hint="Used to find cover art.">
        <div className="flex gap-2">
          <Input
            id={`${id}-folder`}
            readOnly
            placeholder="None chosen"
            value={installFolder}
            className="flex-1 font-mono text-xs"
          />
          <Button variant="outline" size="icon" aria-label="Choose the install folder" onClick={chooseFolder}>
            <Folder className="size-4" />
          </Button>
        </div>
      </Field>
    </div>
  );
}
