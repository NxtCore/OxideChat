import type {AssignmentTarget} from './generated/AssignmentTarget';

export type {AssignmentKind} from './generated/AssignmentKind';
export type {AssignmentRole} from './generated/AssignmentRole';
export type {AssignmentTarget} from './generated/AssignmentTarget';
export type {AssignmentsResponse} from './generated/AssignmentsResponse';

export type AssignmentOption = Pick<AssignmentTarget, 'kind' | 'id' | 'label'>;
