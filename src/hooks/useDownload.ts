import { useCallback, useRef, useState } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { toast } from "@heroui/react";
import { downloadSample, downloadPack, DownloadProgress, DownloadSampleRequest, DownloadPackRequest, getDownloadDir } from "../native";

// HeroUI toast types (simplified for compatibility)
interface ToastOptions {
  description?: React.ReactNode;
  duration?: number;
  onClose?: () => void;
}

type DownloadTask = {
  id: string;
  packName?: string;
  sampleName: string;
  progress: number;
  status: DownloadProgress['status'];
  error?: string;
};

export function useDownload() {
  const [activeTasks, setActiveTasks] = useState<DownloadTask[]>([]);
  const [isDownloadingPack, setIsDownloadingPack] = useState(false);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const taskProgressRef = useRef<Map<string, DownloadProgress>>(new Map());

  // Initialize listener
  const initListener = useCallback(async () => {
    if (unlistenRef.current) return;

    unlistenRef.current = await listen<DownloadProgress>("download-progress", (event) => {
      const progress = event.payload;
      taskProgressRef.current.set(progress.task_id, progress);

      setActiveTasks(prev => {
        const existing = prev.find(t => t.id === progress.task_id);
        const task: DownloadTask = {
          id: progress.task_id,
          packName: progress.pack_name,
          sampleName: progress.sample_name,
          progress: progress.total > 0 ? (progress.current / progress.total) * 100 : 0,
          status: progress.status,
          error: progress.error,
        };

        if (existing) {
          return prev.map(t => t.id === progress.task_id ? task : t);
        }
        return [...prev, task];
      });

      // Show toast for completion/errors
      if (progress.status === 'completed' && !progress.pack_name) {
        toast.success("Download completo", {
          description: progress.sample_name,
        });
      } else if (progress.status === 'completed' && progress.pack_name && progress.current === progress.total) {
        const total = progress.total;
        const error = progress.error;
        if (error) {
          toast.warning(`Pack "${progress.pack_name}" concluído com erros`, {
            description: `${total} samples, ${error}`,
          });
        } else {
          toast.success(`Pack "${progress.pack_name}" baixado!`, {
            description: `${total} samples`,
          });
        }
      } else if (progress.status === 'failed') {
        toast.danger("Falha no download", {
          description: `${progress.sample_name}: ${progress.error || "Erro desconhecido"}`,
        });
      }
    });
  }, []);

  // Cleanup
  const cleanup = useCallback(() => {
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }
    taskProgressRef.current.clear();
    setActiveTasks([]);
  }, []);

  // Download single sample
  const handleDownloadSample = useCallback(async (
    sampleUuid: string,
    sampleName: string,
    packName: string | undefined,
    previewUrl: string,
    packUuid?: string
  ) => {
    await initListener();

    const request: DownloadSampleRequest = {
      sample_uuid: sampleUuid,
      sample_name: sampleName,
      pack_name: packName,
      url: previewUrl,
      pack_uuid: packUuid,
    };

    try {
      await downloadSample(request);
    } catch (err) {
      toast.danger("Falha ao iniciar download", {
        description: err instanceof Error ? err.message : String(err),
      } as ToastOptions);
    }
  }, [initListener]);

  // Download entire pack
  const handleDownloadPack = useCallback(async (
    packUuid: string,
    packName: string,
    sampleUuids: string[],
    baseUrl: string = "https://surfaces-graphql.splice.com/graphql"
  ) => {
    await initListener();
    setIsDownloadingPack(true);

    const request: DownloadPackRequest = {
      pack_uuid: packUuid,
      pack_name: packName,
      sample_uuids: sampleUuids,
      base_url: baseUrl,
    };

    try {
      await downloadPack(request);
    } catch (err) {
      toast.danger("Falha ao baixar pack", {
        description: err instanceof Error ? err.message : String(err),
      } as ToastOptions);
    } finally {
      setIsDownloadingPack(false);
    }
  }, [initListener]);

  // Get download directory
  const handleGetDownloadDir = useCallback(async () => {
    try {
      return await getDownloadDir();
    } catch (err) {
      toast.danger("Erro ao obter pasta de downloads", {
        description: err instanceof Error ? err.message : String(err),
      } as ToastOptions);
      return null;
    }
  }, []);

  // Remove completed task from list
  const removeTask = useCallback((taskId: string) => {
    setActiveTasks(prev => prev.filter(t => t.id !== taskId));
    taskProgressRef.current.delete(taskId);
  }, []);

  // Clear all tasks
  const clearTasks = useCallback(() => {
    setActiveTasks([]);
    taskProgressRef.current.clear();
  }, []);

  return {
    activeTasks,
    isDownloadingPack,
    downloadSample: handleDownloadSample,
    downloadPack: handleDownloadPack,
    getDownloadDir: handleGetDownloadDir,
    removeTask,
    clearTasks,
    initListener,
    cleanup,
  };
}