import { createFileRoute, notFound } from "@tanstack/react-router";
import { GrowthDashboard } from "../growth/GrowthDashboard";
import { validGrowthId } from "../growth/view";

export const Route = createFileRoute("/growth/$battleId")({ beforeLoad: ({ params }) => { if (!validGrowthId(params.battleId)) throw notFound(); }, component: BattlePage });
function BattlePage() { const { battleId } = Route.useParams(); return <GrowthDashboard battleId={battleId} />; }
