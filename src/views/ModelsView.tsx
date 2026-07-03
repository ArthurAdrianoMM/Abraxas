import { useEffect } from "react";
import { CatalogPane } from "../components/models/CatalogPane";
import { DownloadPane } from "../components/models/DownloadPane";
import { ManagerPane } from "../components/models/ManagerPane";
import { useCatalogStore } from "../stores/catalog";
import { useModelStore } from "../stores/model";
import { useUiStore } from "../stores/ui";

export function ModelsView() {
  const pane = useUiStore((s) => s.modelsPane);
  const initModel = useModelStore((s) => s.init);
  const catalogStatus = useCatalogStore((s) => s.status);
  const refreshCatalog = useCatalogStore((s) => s.refresh);

  useEffect(() => {
    void initModel();
    // The manager enriches rows with catalog metadata, so warm it here too.
    if (catalogStatus === "idle") void refreshCatalog();
  }, [initModel, catalogStatus, refreshCatalog]);

  if (pane === "download") return <DownloadPane />;
  if (pane === "catalog") return <CatalogPane />;
  return <ManagerPane />;
}
