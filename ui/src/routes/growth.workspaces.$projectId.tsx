import { createFileRoute, notFound } from "@tanstack/react-router";
import { GrowthDashboard } from "../growth/GrowthDashboard";
import { validGrowthId } from "../growth/view";

export const Route = createFileRoute("/growth/workspaces/$projectId")({ beforeLoad: ({ params }) => { if (!validGrowthId(params.projectId)) throw notFound(); }, component: WorkspacePage });
function WorkspacePage() { const { projectId } = Route.useParams(); return <GrowthDashboard projectId={projectId} />; }
