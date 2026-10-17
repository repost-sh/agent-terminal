import { expect, test } from "vitest";

import { sessionInfo, tabInfo } from "../fake-daemon/messages";
import { routeAfterRenames } from "./renames";
import { parseRoute } from "./route";

const sessions = [
  sessionInfo("work", [tabInfo("app"), tabInfo("web", { renamed_from: "api" })]),
  sessionInfo("play", [tabInfo("api")]),
];

test("a route to a tab that was renamed moves to its new name, and every other route stays", () => {
  expect(
    ["/s/work/api", "/s/work/app", "/s/play/api", "/s/work/gone", "/split/work:api,work:app", "/split/play:api,_", "/split/_,work:gone", "/", "/nowhere"].map(
      (path) => routeAfterRenames(parseRoute(path), sessions),
    ),
  ).toEqual(["/s/work/web", null, null, null, "/split/work:web,work:app", null, null, null, null]);
});
