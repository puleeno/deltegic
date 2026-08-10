import { Router, type IRouter } from "express";
import healthRouter from "./health";
import tasksRouter from "./tasks";
import addonsRouter from "./addons";
import accountsRouter from "./accounts";
import schedulerRouter from "./scheduler";
import settingsRouter from "./settings";

const router: IRouter = Router();

router.use(healthRouter);
router.use(tasksRouter);
router.use(addonsRouter);
router.use(accountsRouter);
router.use(schedulerRouter);
router.use(settingsRouter);

export default router;
