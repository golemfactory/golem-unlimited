angular.module('gu').directive('guSelectProvider', function () {
    return {
        restrict: 'E',
        scope: {
            peers: '=ngModel'
        },
        require: 'ngModel',
        templateUrl: 'directives/guSelectProvider.html',
        controller: function ($scope, sessionMan) {
            console.log('new-x', $scope);
            $scope.hubPeers = [];
            $scope.statusMap = {};

            _.forEach($scope.peers, nodeId => {
                $scope.statusMap[nodeId] = true;
            });

            $scope.$watch('peers', (v, ov) => {
                $scope.statusMap = {};
                _.forEach(v, nodeId => {
                    $scope.statusMap[nodeId] = true;
                });
            });

            $scope.isSelected = function(peer) {
                return _.contains($scope.peers, peer.nodeId);
            };

            $scope.toggle = function(peer) {
                let peers = $scope.peers;
                let idx = null;
                for (let i=0; i<peers.length; ++i) {
                    if (peers[i] === peer.nodeId) {
                        idx = i;
                        break;
                    }
                }

                if (idx === null) {
                    peers.push(peer.nodeId);
                }
                else {
                    peers.splice(idx,1);
                }
            };

            sessionMan.allPeers().then(peers => $scope.hubPeers = peers);

        }
    }
});