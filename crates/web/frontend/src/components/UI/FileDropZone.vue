<script setup lang="ts">
/**
 * English note.
 * English note.
 */

import { ref, computed } from "vue";

interface Props {
  // English engineering note.
  multiple?: boolean;
  // English engineering note.
  disabled?: boolean;
  // English engineering note.
  accept?: string[];
}

const props = withDefaults(defineProps<Props>(), {
  multiple: false,
  disabled: false,
  accept: () => ["*"],
});

const emit = defineEmits<{
  // English engineering note.
  files: [payload: { files: File[]; paths: string[] }];
}>();

// English engineering note.
const isDragOver = ref(false);

// English engineering note.
const fileInputRef = ref<HTMLInputElement | null>(null);

// English engineering note.
const acceptAttr = computed(() => {
  if (props.accept.includes("*")) return "*";
  return props.accept.join(",");
});

// English engineering note.
function openFileDialog() {
  if (props.disabled) return;
  fileInputRef.value?.click();
}

// English engineering note.
function handleFileSelect(event: Event) {
  const input = event.target as HTMLInputElement;
  if (!input.files || input.files.length === 0) return;

  processFiles(Array.from(input.files));

  // English engineering note.
  input.value = "";
}

// English engineering note.
function handleDragEnter(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (props.disabled) return;
  isDragOver.value = true;
}

// English engineering note.
function handleDragOver(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (props.disabled) return;
  isDragOver.value = true;
}

// English engineering note.
function handleDragLeave(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  isDragOver.value = false;
}

// English engineering note.
function handleDrop(e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  isDragOver.value = false;

  if (props.disabled) return;

  const dataTransfer = e.dataTransfer;
  if (!dataTransfer?.files || dataTransfer.files.length === 0) return;

  let files = Array.from(dataTransfer.files);

  // English engineering note.
  if (!props.multiple) {
    files = [files[0]];
  }

  // English engineering note.
  if (!props.accept.includes("*")) {
    files = files.filter((file) => {
      const ext = "." + file.name.split(".").pop()?.toLowerCase();
      return props.accept.some((a) => a.toLowerCase() === ext);
    });
  }

  if (files.length > 0) {
    processFiles(files);
  }
}

// English engineering note.
function processFiles(files: File[]) {
  const paths: string[] = [];

  // English engineering note.
  for (const file of files) {
    try {
      const path = window.electron?.webUtils?.getPathForFile?.(file);
      if (path) {
        paths.push(path);
      }
    } catch {
      // English engineering note.
    }
  }

  emit("files", { files, paths });
}

// English engineering note.
defineExpose({
  openFileDialog,
});
</script>

<template>
  <div
    @dragenter="handleDragEnter"
    @dragover="handleDragOver"
    @dragleave="handleDragLeave"
    @drop="handleDrop"
  >
    <!-- English UI note -->
    <input
      ref="fileInputRef"
      type="file"
      :multiple="multiple"
      :accept="acceptAttr"
      class="hidden"
      @change="handleFileSelect"
    />

    <!-- English UI note -->
    <slot
      :is-drag-over="isDragOver"
      :open-file-dialog="openFileDialog"
      :disabled="disabled"
    />
  </div>
</template>
