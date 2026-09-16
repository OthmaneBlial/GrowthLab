import { createFileRoute } from "@tanstack/react-router";
import { GrowthDashboard } from "../growth/GrowthDashboard";

export const Route = createFileRoute("/")({ component: GrowthDashboard });
